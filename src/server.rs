use bytes::Bytes;
use dashmap::DashMap;
use pyo3::prelude::*;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::logging::{log_py, queue_log};

type SharedStore = Arc<DashMap<String, Bytes>>;

async fn handle_client(
    mut stream: TcpStream,
    store: SharedStore,
    logger: Arc<Mutex<Option<PyObject>>>,
) {
    let addr = stream.peer_addr().unwrap();
    let _ = stream.set_nodelay(true);
    log_py(
        &logger,
        "debug",
        &format!("Accepted connection from {:?}", addr),
    );

    let (reader, writer) = stream.split();
    let mut reader = BufReader::new(reader).lines();
    let mut writer = BufWriter::new(writer);

    while let Ok(Some(line)) = reader.next_line().await {
        let mut response: Vec<u8> = Vec::new();

        if line.starts_with("SET ") {
            if let Some((k, v)) = line[4..].split_once(' ') {
                store.insert(k.to_string(), Bytes::from(v.to_owned()));
                response.extend_from_slice(b"OK\n");
            } else {
                response.extend_from_slice(b"ERROR Invalid SET command\n");
            }
        } else if line.starts_with("GET ") {
            let key = &line[4..];
            if let Some(val) = store.get(key) {
                response.extend_from_slice(val.value());
                response.push(b'\n');
            } else {
                response.extend_from_slice(b"ERROR Key not found\n");
            }
        } else if line.starts_with("DEL ") {
            let key = &line[4..];
            if store.remove(key).is_some() {
                store.shrink_to_fit();
                response.extend_from_slice(b"OK\n");
            } else {
                response.extend_from_slice(b"ERROR Key not found\n");
            }
        } else if line.trim() == "GKC" {
            let count = store.len();
            response.extend_from_slice(count.to_string().as_bytes());
            response.push(b'\n');
        } else if line.trim() == "GAK" {
            // get all keys as csv
            let keys: Vec<String> = store.iter().map(|r| r.key().clone()).collect();
            response.extend_from_slice(keys.join(" ").as_bytes());
            response.push(b'\n');
        } else if line.trim() == "QUIT" {
            log_py(&logger, "debug", &format!("Quit requested from {:?}", addr));
            stream.shutdown().await.unwrap();
            break;
        } else if line.trim() == "PING" {
            response.extend_from_slice(b"PONG\n");
        } else {
            log_py(
                &logger,
                "warn",
                &format!("Invalid command from {:?}: {}", addr, line),
            );
            response.extend_from_slice(b"ERROR Invalid command or arguments\n");
        }

        if writer.write_all(&response).await.is_err() {
            break;
        }
        let _ = writer.flush().await;
    }

    log_py(
        &logger,
        "debug",
        &format!("Closing connection from {:?}", addr),
    );
}

#[pyclass]
pub struct NetworkServer {
    host: String,
    port: u16,
    running: Arc<Mutex<bool>>,
    logger: Arc<Mutex<Option<PyObject>>>,
    shutdown: Arc<AtomicBool>,
    thread_handle: Arc<Mutex<Option<std::thread::JoinHandle<()>>>>,
    log_tx: Arc<Mutex<Option<UnboundedSender<(String, String)>>>>,
}

#[pymethods]
impl NetworkServer {
    #[new]
    fn new(host: String, port: u16) -> Self {
        let mut server = Self {
            host,
            port,
            running: Arc::new(Mutex::new(false)),
            logger: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
            log_tx: Arc::new(Mutex::new(None)),
        };

        Python::with_gil(|py| {
            let print_fn = py
                .import("builtins")
                .unwrap()
                .getattr("print")
                .unwrap()
                .into();
            server.set_logger(py, print_fn);
        });

        server
    }

    #[setter]
    fn set_logger(&mut self, py: Python, callback: PyObject) {
        *self.logger.lock().unwrap() = Some(callback.clone_ref(py));

        let logger = self.logger.clone();
        let (tx, mut rx): (
            UnboundedSender<(String, String)>,
            UnboundedReceiver<(String, String)>,
        ) = tokio::sync::mpsc::unbounded_channel();

        *self.log_tx.lock().unwrap() = Some(tx);

        pyo3_async_runtimes::tokio::run(py, async move {
            tokio::spawn(async move {
                while let Some((level, msg)) = rx.recv().await {
                    Python::with_gil(|py| {
                        if let Some(cb) = logger.lock().unwrap().as_ref() {
                            let _ = cb.call1(py, (level.as_str(), msg.as_str()));
                        }
                    });
                }
            });
            Ok(())
        })
        .unwrap();
    }

    fn start(&self) {
        let addr = match format!("{}:{}", self.host, self.port).parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                queue_log(&self.log_tx, "error", &format!("Invalid address: {}", e));
                return;
            }
        };

        let is_running = self.running.clone();
        let shutdown = self.shutdown.clone();
        let log_tx = self.log_tx.clone();

        {
            let mut flag = is_running.lock().unwrap();
            if *flag {
                queue_log(&log_tx, "warning", "Server already running");
                return;
            }
            *flag = true;
            shutdown.store(false, Ordering::SeqCst);
        }

        let logger = self.logger.clone();

        let thread_handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .worker_threads(num_cpus::get())
                .enable_all()
                .build()
                .unwrap();

            let store: SharedStore = Arc::new(DashMap::new());
            let logger_future = logger.clone();
            let shutdown_flag = shutdown.clone();
            let shutdown_flag_inner = shutdown_flag.clone();
            let running_flag = is_running.clone();
            let log_tx_future = log_tx.clone();

            let server_future = async move {
                let listener = TcpListener::bind(addr).await.unwrap();
                queue_log(&log_tx, "debug", &format!("Serving on {}", addr));

                loop {
                    let accept_result = tokio::select! {
                        _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => {
                            if shutdown_flag_inner.load(Ordering::SeqCst) {
                                store.clear();
                                break;
                            }
                            continue;
                        }
                        result = listener.accept() => result,
                    };

                    let (stream, _) = match accept_result {
                        Ok(s) => s,
                        Err(e) => {
                            queue_log(&log_tx, "warning", &format!("Accept failed: {}", e));
                            continue;
                        }
                    };

                    let store = store.clone();
                    let client_logger = logger_future.clone();
                    tokio::spawn(handle_client(stream, store, client_logger));
                }

                Ok::<(), std::io::Error>(())
            };

            if let Err(e) = rt.block_on(server_future) {
                queue_log(&log_tx_future, "error", &format!("Server error: {}", e));
            }

            *running_flag.lock().unwrap() = false;
            shutdown_flag.store(false, Ordering::SeqCst);
        });

        *self.thread_handle.lock().unwrap() = Some(thread_handle);
    }

    fn stop(&self) {
        self.shutdown.store(true, Ordering::SeqCst);
        queue_log(&self.log_tx, "debug", "Shutting down server...");
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    #[getter]
    fn running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

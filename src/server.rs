use pyo3::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::RwLock;

use crate::logging::{log_py, queue_log};

type SharedStore = Arc<RwLock<HashMap<String, String>>>;

async fn handle_client(
    stream: TcpStream,
    store: SharedStore,
    logger: Arc<Mutex<Option<PyObject>>>,
) {
    let addr = stream.peer_addr().unwrap();
    log_py(
        &logger,
        "debug",
        &format!("Accepted connection from {:?}", addr),
    );

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader).lines();

    while let Ok(Some(line)) = reader.next_line().await {
        let parts: Vec<&str> = line.trim().splitn(3, ' ').collect();
        let command = parts.get(0).map(|s| s.to_uppercase());
        let key = parts.get(1).copied();
        let value = parts.get(2).copied();

        let mut response = String::new();

        match (command.as_deref(), key, value) {
            (Some("SET"), Some(k), Some(v)) => {
                store.write().await.insert(k.to_string(), v.to_string());
                response.push_str("OK\n");
            }
            (Some("GET"), Some(k), _) => {
                if let Some(val) = store.read().await.get(k) {
                    response.push_str(val);
                    response.push('\n');
                } else {
                    response.push_str("ERROR Key not found\n");
                }
            }
            (Some("DEL"), Some(k), _) => {
                if let Some(_) = store.write().await.remove(k) {
                    response.push_str("OK\n");
                } else {
                    response.push_str("ERROR Key not found\n");
                }
            }
            _ => {
                log_py(
                    &logger,
                    "warn",
                    &format!("Invalid command from {:?}: {}", addr, line),
                );
                response.push_str("ERROR Invalid command or arguments\n");
            }
        }

        if writer.write_all(response.as_bytes()).await.is_err() {
            break;
        }
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
        Self {
            host,
            port,
            running: Arc::new(Mutex::new(false)),
            logger: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(AtomicBool::new(false)),
            thread_handle: Arc::new(Mutex::new(None)),
            log_tx: Arc::new(Mutex::new(None)),
        }
    }

    #[setter]
    fn set_logger(&mut self, py: Python, callback: PyObject) {
        *self.logger.lock().unwrap() = Some(callback.clone_ref(py));

        // Start the log drain task
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

    /// Start the server
    fn start(&self) {
        let addr = match format!("{}:{}", self.host, self.port).parse::<SocketAddr>() {
            Ok(addr) => addr,
            Err(e) => {
                self.log("error", &format!("Invalid address: {}", e));
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
                .enable_all()
                .build()
                .unwrap();

            let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
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
                                store.write().await.clear();
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
                    stream.set_nodelay(true)?;

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
        self.log("debug", "Shutdown requested");
        if let Some(handle) = self.thread_handle.lock().unwrap().take() {
            let _ = handle.join();
        }
    }

    #[getter]
    fn running(&self) -> bool {
        *self.running.lock().unwrap()
    }
}

impl NetworkServer {
    /// Instance method for logging
    fn log(&self, level: &str, message: &str) {
        if let Some(cb) = self.logger.lock().unwrap().as_ref() {
            Python::with_gil(|py| {
                let _ = cb.call1(py, (level, message));
            });
        }
    }
}

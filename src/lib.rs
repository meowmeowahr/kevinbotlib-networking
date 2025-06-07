use pyo3::prelude::*;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::RwLock;

type SharedStore = Arc<RwLock<HashMap<String, String>>>;

async fn handle_client(stream: TcpStream, store: SharedStore, logger: Arc<Mutex<Option<PyObject>>>) {
    let addr = stream.peer_addr().unwrap();
    log_py(&logger, "info", &format!("Accepted connection from {:?}", addr));

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
            _ => {
                log_py(&logger, "warn", &format!("Invalid command from {:?}: {}", addr, line));
                response.push_str("ERROR Invalid command or arguments\n");
            }
        }

        if writer.write_all(response.as_bytes()).await.is_err() {
            break;
        }
    }
    
    log_py(&logger, "info", &format!("Closing connection from {:?}", addr));
}

#[pyclass]
pub struct NetworkServer {
    host: String,
    port: u16,
    running: Arc<Mutex<bool>>,
    logger: Arc<Mutex<Option<PyObject>>>,
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
        }
    }

    #[setter]
    fn set_logger(&mut self, py: Python, callback: PyObject) {
        *self.logger.lock().unwrap() = Some(callback.clone_ref(py));
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
        {
            let mut flag = is_running.lock().unwrap();
            if *flag {
                self.log("warn", "Server already running");
                return;
            }
            *flag = true;
        }

        let logger = self.logger.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();

            let store: SharedStore = Arc::new(RwLock::new(HashMap::new()));
            let logger_future = logger.clone();

            let server_future = async move {
                let listener = TcpListener::bind(addr).await.unwrap();
                log_py(&logger_future, "info", &format!("Serving on {}", addr));

                loop {
                    let (stream, _) = listener.accept().await?;
                    let store = store.clone();
                    let client_logger = logger_future.clone();
                    tokio::spawn(handle_client(stream, store, client_logger));
                }

                #[allow(unreachable_code)]
                Ok::<(), std::io::Error>(())
            };

            if let Err(e) = rt.block_on(server_future) {
                log_py(&logger, "error", &format!("Server error: {}", e));
            }

            *is_running.lock().unwrap() = false;
        });
    }

    /// Python property: is the server running?
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

/// Static-style helper for use inside `tokio::spawn` closures
fn log_py(logger: &Arc<Mutex<Option<PyObject>>>, level: &str, msg: &str) {
    if let Some(cb) = logger.lock().unwrap().as_ref() {
        Python::with_gil(|py| {
            if let Err(e) = cb.call1(py, (level, msg)) {
                e.print(py);  // ✅ Will print traceback to stderr
            }
        });
    }
}


#[pymodule]
fn kevinbotlib_networking(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<NetworkServer>()?;
    Ok(())
}
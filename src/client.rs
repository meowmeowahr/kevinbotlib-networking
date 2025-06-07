use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::{Duration};
use pyo3::{pyclass, pymethods, PyObject, Python};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

#[pyclass]
pub struct BlockingClient {
    host: String,
    port: u16,
    stream: Option<TcpStream>,
    logger: Arc<Mutex<Option<PyObject>>>,
    log_tx: Arc<Mutex<Option<UnboundedSender<(String, String)>>>>,
}

#[pymethods]
impl BlockingClient {
    #[new]
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
            stream: None,
            logger: Arc::new(Mutex::new(None)),
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

    pub fn connect(&mut self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;
        self.stream = Some(stream);
        println!("Connected to server.");
        Ok(())
    }

    pub fn disconnect(&mut self) {
        if let Some(stream) = self.stream.take() {
            drop(stream);
            println!("Disconnected from server.");
        }
    }

    fn send_receive(&mut self, message: &str) -> std::io::Result<String> {
        let stream = self.stream.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "Client not connected")
        })?;

        stream.write_all(message.as_bytes())?;
        stream.write_all(b"\n")?;
        stream.flush()?;

        let mut reader = BufReader::new(stream.try_clone()?);
        let mut response = String::new();
        reader.read_line(&mut response)?;
        Ok(response.trim_end().to_string())
    }

    pub fn set(&mut self, key: &str, value: &str) -> bool {
        match self.send_receive(&format!("SET {} {}", key, value)) {
            Ok(resp) => resp == "OK",
            Err(e) => {
                eprintln!("SET error: {}", e);
                false
            }
        }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        match self.send_receive(&format!("GET {}", key)) {
            Ok(resp) => {
                if resp.starts_with("ERROR") {
                    eprintln!("GET error: {}", resp);
                    None
                } else {
                    Some(resp)
                }
            }
            Err(e) => {
                eprintln!("GET error: {}", e);
                None
            }
        }
    }

    pub fn delete(&mut self, key: &str) -> bool {
        match self.send_receive(&format!("DEL {}", key)) {
            Ok(resp) => resp == "OK",
            Err(e) => {
                eprintln!("SET error: {}", e);
                false
            }
        }
    }
}

impl BlockingClient {
    /// Instance method for logging
    fn log(&self, level: &str, message: &str) {
        if let Some(cb) = self.logger.lock().unwrap().as_ref() {
            Python::with_gil(|py| {
                let _ = cb.call1(py, (level, message));
            });
        }
    }
}


// fn main() {
//     const SERVER_HOST: &str = "127.0.0.1";
//     const SERVER_PORT: u16 = 8888;
//     const NUM_OPERATIONS: usize = 1000;
//     const VALUE_SIZE: usize = 100;
//
//     let data = random_string(VALUE_SIZE);
//
//     let mut client = BlockingClient::new(SERVER_HOST, SERVER_PORT);
//     if let Err(e) = client.connect() {
//         eprintln!("Failed to connect: {}", e);
//         return;
//     }
//
//     // SET operations
//     println!("Starting {} SET operations...", NUM_OPERATIONS);
//     let start = Instant::now();
//     let mut set_success = 0;
//     for i in 0..NUM_OPERATIONS {
//         let key = format!("key_{}", i);
//         if client.set(&key, &data) {
//             set_success += 1;
//         }
//         if i % (NUM_OPERATIONS / 10).max(1) == 0 {
//             println!("  {}/{} SETs completed...", i, NUM_OPERATIONS);
//         }
//     }
//     let duration = start.elapsed();
//     println!(
//         "Completed SETs in {:.4}s, success: {} / {}, avg: {:.4}ms",
//         duration.as_secs_f64(),
//         set_success,
//         NUM_OPERATIONS,
//         duration.as_secs_f64() * 1000.0 / set_success.max(1) as f64
//     );
//
//     // GET operations
//     println!("Starting {} GET operations...", NUM_OPERATIONS);
//     let start = Instant::now();
//     let mut get_success = 0;
//     for i in 0..NUM_OPERATIONS {
//         let key = format!("key_{}", i);
//         if let Some(_val) = client.get(&key) {
//             get_success += 1;
//         }
//         if i % (NUM_OPERATIONS / 10).max(1) == 0 {
//             println!("  {}/{} GETs completed...", i, NUM_OPERATIONS);
//         }
//     }
//     let duration = start.elapsed();
//     println!(
//         "Completed GETs in {:.4}s, success: {} / {}, avg: {:.4}ms",
//         duration.as_secs_f64(),
//         get_success,
//         NUM_OPERATIONS,
//         duration.as_secs_f64() * 1000.0 / get_success.max(1) as f64
//     );
//
//     client.disconnect();
// }

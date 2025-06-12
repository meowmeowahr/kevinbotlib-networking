use pyo3::prelude::*;
use pyo3::{pyclass, pymethods, PyObject, Python};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::net::TcpStream;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use crate::logging::queue_log;

struct StreamWrapper {
    reader: BufReader<TcpStream>,
    writer: BufWriter<TcpStream>,
}

#[pyclass]
pub struct BlockingClient {
    host: String,
    port: u16,
    stream_wrapper: Option<StreamWrapper>,
    logger: Arc<Mutex<Option<PyObject>>>,
    log_tx: Arc<Mutex<Option<UnboundedSender<(String, String)>>>>,
}

#[pymethods]
impl BlockingClient {
    #[new]
    pub fn new(host: &str, port: u16) -> Self {
        let mut client = Self {
            host: host.to_string(),
            port,
            stream_wrapper: None,
            logger: Arc::new(Mutex::new(None)),
            log_tx: Arc::new(Mutex::new(None)),
        };

        Python::with_gil(|py| {
            let print_fn = py.import("builtins").unwrap().getattr("print").unwrap().into();
            client.set_logger(py, print_fn);
        });

        client
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

    pub fn connect(&mut self) -> std::io::Result<()> {
        let addr = format!("{}:{}", self.host, self.port);
        let stream = TcpStream::connect(addr)?;
        stream.set_nodelay(true)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let reader = BufReader::new(stream.try_clone()?);
        let writer = BufWriter::new(stream);

        self.stream_wrapper = Some(StreamWrapper { reader, writer });
        Ok(())
    }

    pub fn close(&mut self) {
        self.stream_wrapper = None;
    }

    #[getter]
    pub fn connected(&self) -> bool {
        self.stream_wrapper.is_some()
    }

    pub fn set(&mut self, key: &str, value: &str) -> bool {
        let message = format!("SET {} {}\n", key, value);
        match self.send_receive(&message) {
            Ok(resp) => resp == "OK",
            Err(e) => {
                Err(e).unwrap()
            }
        }
    }

    pub fn get(&mut self, key: &str) -> Option<String> {
        let message = format!("GET {}\n", key);
        match self.send_receive(&message) {
            Ok(resp) => {
                Some(resp)
            }
            Err(e) => {
                Err(e).unwrap()
            }
        }
    }

    pub fn delete(&mut self, key: &str) -> bool {
        let message = format!("DEL {}\n", key);
        match self.send_receive(&message) {
            Ok(resp) => resp == "OK",
            Err(e) => {
                Err(e).unwrap()
            }
        }
    }

    pub fn get_all_keys(&mut self) -> Vec<String> {
        let message = "GAK\n".to_string();
        match self.send_receive(&message) {
            Ok(resp) => {
                resp.split_whitespace().map(|s| s.to_string()).collect()
            },
            Err(e) => {
                Err(e).unwrap()
            }
        }
    }

    pub fn get_key_count(&mut self) -> u64 {
        let message = "GKC\n".to_string();
        match self.send_receive(&message) {
            Ok(resp) => {
                resp.parse::<u64>().unwrap_or(0)
            },
            Err(e) => {
                Err(e).unwrap()
            }
        }
    }

    pub fn send_command(&mut self, command: &str) -> std::io::Result<String> {
        let message = format!("{}\n", command);
        self.send_receive(&message)
    }
}

impl BlockingClient {
    fn send_receive(&mut self, message: &str) -> std::io::Result<String> {
        let stream_wrapper = self.stream_wrapper.as_mut().ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotConnected, "Client not connected")
        })?;

        // Write
        stream_wrapper.writer.write_all(message.as_bytes())?;
        stream_wrapper.writer.flush()?;

        // Read
        let mut response = String::new();
        stream_wrapper.reader.read_line(&mut response)?;

        Ok(response.trim_end().to_string())
    }
}

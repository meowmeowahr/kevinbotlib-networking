use pyo3::{PyObject, Python};
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc::UnboundedSender;

pub fn queue_log(
    log_tx: &Arc<Mutex<Option<UnboundedSender<(String, String)>>>>,
    level: &str,
    msg: &str,
) {
    if let Some(sender) = log_tx.lock().unwrap().as_ref() {
        let _ = sender.send((level.to_string(), msg.to_string()));
    }
}

pub fn log_py(logger: &Arc<Mutex<Option<PyObject>>>, level: &str, msg: &str) {
    if let Some(cb) = logger.lock().unwrap().as_ref() {
        Python::with_gil(|py| {
            if let Err(e) = cb.call1(py, (level, msg)) {
                e.print(py); // ✅ Will print traceback to stderr
            }
        });
    }
}

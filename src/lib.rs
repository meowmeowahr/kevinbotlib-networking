mod logging;
mod server;
mod client;

use pyo3::prelude::*;

/// Static-style helper for use inside `tokio::spawn` closures

#[pymodule]
fn kevinbotlib_networking(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<server::NetworkServer>()?;
    m.add_class::<client::BlockingClient>()?;
    Ok(())
}

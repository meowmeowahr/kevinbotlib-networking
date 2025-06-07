mod logging;
mod server;

use pyo3::prelude::*;

/// Static-style helper for use inside `tokio::spawn` closures

#[pymodule]
fn kevinbotlib_networking(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<server::NetworkServer>()?;
    Ok(())
}

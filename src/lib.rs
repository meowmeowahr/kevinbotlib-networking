use pyo3::prelude::*;
use pyo3::wrap_pyfunction;
mod server;

#[pyfunction]
fn start_server(host: String, port: u16) -> PyResult<()> {
    // Wrap async call in a new Tokio runtime
    let rt = tokio::runtime::Runtime::new().map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Failed to create runtime: {}", e))
    })?;
    rt.block_on(async {
        server::start_server(&host, port).await.map_err(|e| {
            PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Start failed: {}", e))
        })
    })?;
    Ok(())
}

#[pyfunction]
fn stop_server() -> PyResult<()> {
    server::stop_server().map_err(|e| {
        PyErr::new::<pyo3::exceptions::PyRuntimeError, _>(format!("Stop failed: {}", e))
    })?;
    Ok(())
}

#[pymodule]
fn kevinbotlib_networking(_py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(start_server, m)?)?;
    m.add_function(wrap_pyfunction!(stop_server, m)?)?;
    Ok(())
}

use pyo3::exceptions::PyRuntimeError;
use pyo3::{prelude::*, types::PyBytes};

#[pyfunction]
fn read_tile(py: Python, filename: String, z: u32, x: u64, y: u64) -> PyResult<&PyAny> {
    use ::acog::tiler::{extract_tile, TMSTileCoords};

    pyo3_asyncio::tokio::future_into_py(py, async move {
        let mut cog = match ::acog::COG::open(&filename).await {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };

        let tile_data = match extract_tile(&mut cog, TMSTileCoords::from_zxy(z, x, y)).await {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };
        let tile_data_py =
            pyo3::Python::with_gil(|py| PyBytes::new(py, &tile_data.data).to_object(py));
        Ok(tile_data_py)
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn acog(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_tile, m)?)?;
    Ok(())
}

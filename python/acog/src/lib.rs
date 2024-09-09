use ::acog::tiler::TileData;
use pyo3::exceptions::PyRuntimeError;
use pyo3::{prelude::*, types::PyBytes};

#[pyclass]
struct ImageTile {
    tile_data: TileData,
}

#[pymethods]
impl ImageTile {
    fn data_buffer(&self, py: Python) -> PyResult<Py<PyAny>> {
        let tile_data_py = PyBytes::new(py, &self.tile_data.img.data).to_object(py);
        Ok(tile_data_py)
    }

    fn width(&self) -> PyResult<usize> {
        Ok(self.tile_data.img.width)
    }

    fn height(&self) -> PyResult<usize> {
        Ok(self.tile_data.img.height)
    }

    fn nbands(&self) -> PyResult<usize> {
        Ok(self.tile_data.img.nbands)
    }
}

/// This returns an `ImageTile`
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
        let tile = ImageTile { tile_data };
        Ok(tile)
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn acog(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_tile, m)?)?;
    m.add_class::<ImageTile>()?;
    Ok(())
}

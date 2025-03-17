use ::acog::image::ImageBuffer;
use ::acog::DataType;
use pyo3::exceptions::PyRuntimeError;
use pyo3::{prelude::*, types::PyBytes};

// TODO: Rename to SubImage ? As this is used both for TMS tiles but also arbitrary subimages
#[pyclass]
struct PyImage {
    img: ImageBuffer,
}

#[pymethods]
impl PyImage {
    fn data_buffer(&self, py: Python) -> PyResult<Py<PyAny>> {
        let tile_data_py = PyBytes::new(py, &self.img.data).to_object(py);
        Ok(tile_data_py)
    }

    fn width(&self) -> PyResult<usize> {
        Ok(self.img.width)
    }

    fn height(&self) -> PyResult<usize> {
        Ok(self.img.height)
    }

    fn nbands(&self) -> PyResult<usize> {
        Ok(self.img.nbands)
    }

    fn dtype(&self) -> PyResult<String> {
        Ok(match self.img.data_type {
            DataType::Uint8 => "uint8".to_string(),
            DataType::Float32 => "float32".to_string(),
        })
    }
}

#[pyclass]
struct BBox(::acog::BoundingBox);

#[pymethods]
impl BBox {
    fn xmin(&self) -> PyResult<f64> {
        Ok(self.0.xmin)
    }

    fn xmax(&self) -> PyResult<f64> {
        Ok(self.0.xmax)
    }

    fn ymin(&self) -> PyResult<f64> {
        Ok(self.0.ymin)
    }

    fn ymax(&self) -> PyResult<f64> {
        Ok(self.0.ymax)
    }
}

// TODO: Rename to read_tms_tile

/// Reads a TMS tile from the given image.
/// Note that this also handles reprojection from the image SRS into EPSG:3857
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
        let tile = PyImage { img: tile_data.img };
        Ok(tile)
    })
}

/// Reads a part of the image in the native image coordinate system (i.e. this doesn't reproject)
/// This returns an `ImageTile`
#[pyfunction]
fn read_subimage(
    py: Python,
    filename: String,
    overview_index: usize,
    x_from: u64,
    y_from: u64,
    x_to: u64,
    y_to: u64,
) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let mut cog = match ::acog::COG::open(&filename).await {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };
        let rect = ::acog::ImageRect {
            i_from: y_from,
            i_to: y_to,
            j_from: x_from,
            j_to: x_to,
        };

        let image_buffer = match cog.read_image_part(overview_index, &rect).await {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };
        let tile = PyImage { img: image_buffer };
        Ok(tile)
    })
}

#[pyfunction]
fn get_bounds(py: Python, filename: String) -> PyResult<&PyAny> {
    pyo3_asyncio::tokio::future_into_py(py, async move {
        let cog = match ::acog::COG::open(&filename).await {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };

        let bbox = match cog.lnglat_bounds() {
            Ok(v) => v,
            Err(e) => return Err(PyRuntimeError::new_err(format!("{:?}", e))),
        };
        Ok(BBox(bbox))
    })
}

/// A Python module implemented in Rust.
#[pymodule]
fn acog(_py: Python, m: &PyModule) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(read_tile, m)?)?;
    m.add_function(wrap_pyfunction!(read_subimage, m)?)?;
    m.add_function(wrap_pyfunction!(get_bounds, m)?)?;
    m.add_class::<PyImage>()?;
    m.add_class::<BBox>()?;
    Ok(())
}

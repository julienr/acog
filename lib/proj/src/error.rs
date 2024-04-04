#[derive(Debug)]
pub enum Error {
    FFIError(String),
    PROJError(i32, String),
    OtherError(String),
}

impl From<std::ffi::NulError> for Error {
    fn from(_value: std::ffi::NulError) -> Self {
        Error::FFIError("Nul byte in the middle of string".to_string())
    }
}

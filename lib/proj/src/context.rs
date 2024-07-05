use crate::bindings;
use crate::error::Error;
use std::ffi::CStr;

pub struct Context {
    context: *mut bindings::PJ_CONTEXT,
}

impl Context {
    pub fn new() -> Context {
        let context = unsafe { bindings::proj_context_create() };
        Context { context }
    }

    /// Returns the latest error as an (error_code, error_string) pair or None if there
    /// are no errors in this context
    pub fn get_error_code_and_string(&self) -> Option<(i32, String)> {
        let err = unsafe { bindings::proj_context_errno(self.context) } as i32;
        if err == 0 {
            None
        } else {
            let string =
                unsafe { CStr::from_ptr(bindings::proj_context_errno_string(self.context, err)) }
                    .to_string_lossy()
                    .to_string();
            Some((err, string))
        }
    }

    pub fn get_error(&self) -> Error {
        match self.get_error_code_and_string() {
            None => Error::OtherError("get_error called but proj returned no error".to_string()),
            Some((code, errstr)) => Error::PROJError(code, errstr),
        }
    }

    pub fn c_ptr(&mut self) -> *mut bindings::PJ_CONTEXT {
        self.context
    }

    // A lot of PROJ functions will take a PJ_CONTEXT* that is not marking it const, but the
    // function won't modify the context. In cases like these, we allow obtaining a *mut PJ_CONTEXT
    // from a &self
    // TODO: Check that this is really a good idea
    pub(super) unsafe fn mut_c_ptr_from_immutable_self(&self) -> *mut bindings::PJ_CONTEXT {
        self.context
    }
}

impl Drop for Context {
    fn drop(&mut self) {
        unsafe { bindings::proj_context_destroy(self.context) };
    }
}

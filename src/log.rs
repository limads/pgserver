use std::ffi::CString;

/// Emits a log using PostgreSQL raise mechanism.
///
/// ```rust
/// log::Notice.raise("Just a friendly notice - carry on");
/// log::Warning.raise("An important warning - but carry on");
/// log::Error.raise("Error executing the function - stop here");
/// ```

pub struct Notice;

impl Notice {
    pub fn raise(msg : &str) {
        let c_msg = CString::new(msg).unwrap();
        let msg_raw = c_msg.into_raw();
        unsafe { super::report(18, msg_raw) }
    }
}

pub struct Warning;

impl Warning {
    pub fn raise(msg : &str) {
        let c_msg = CString::new(msg).unwrap();
        let msg_raw = c_msg.into_raw();
        unsafe { super::report(19, msg_raw) }
    }
}

/// Error differs from Warning and Notice in that it stops execution of
/// the current function, so it can be called at the return point
/// of the final expression of a function.
pub struct Error;

impl Error {
    pub fn raise(msg : &str) -> ! {
        let c_msg = CString::new(msg).unwrap();
        let msg_raw = c_msg.into_raw();
        unsafe { super::report(20, msg_raw) }
        unreachable!()
    }
}

/*impl Log {

    pub fn raise(&self, msg : &str) -> Option<!> {
        let c_msg = CString::new(msg).unwrap();
        let msg_raw = c_msg.into_raw();
        unsafe {
            match self {
                Log::Notice => { super::report(18, msg_raw); None },
                Log::Warning => { super::report(19, msg_raw); None },
                Log::Error => { super::report(20, msg_raw); Some(unreachable!()) }
            }
        }
    }

}*/



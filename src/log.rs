use std::ffi::CString;

pub enum Log {
    Notice,
    Warning,
    Error
}

impl Log {

    pub fn raise(&self, msg : &str) {
        let c_msg = CString::new(msg).unwrap();
        let msg_raw = c_msg.into_raw();
        unsafe {
            match self {
                Log::Notice => super::report(18, msg_raw),
                Log::Warning => super::report(19, msg_raw),
                Log::Error => super::report(20, msg_raw),
            }
        }
    }

}

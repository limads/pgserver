#![feature(vec_into_raw_parts)]
#![feature(c_variadic)]
#![feature(never_type)]

use std::slice;
use std::os::raw::c_char;
use std::fmt;
use std::convert::{self, TryInto};
use std::mem;
use std::ffi::CString;
use std::ptr;

/// Enumeration that wraps PostgreSQL logging via raise
pub mod log;

/// Utilities to build PostgreSQL extensions
pub mod build;

/// Bindgen-generated code to represent variable-length arrays allocated by Postgres.
mod vla;

use vla::varlena;

// Available at link time from the pg_helper.c module (which in turn relies on C calls which
// will be available only at runtime at the server).
extern "C" {

    fn read_from_pg(arg : *const varlena) -> ByteSlice;

    fn palloc_varlena(sz : usize) -> *const varlena;

    fn copy_to_pg(s : ByteSlice) -> *const varlena;

    fn bytes_ptr(t : *const varlena) -> *const u8;

    fn bytes_len(t : *const varlena) -> usize;

    fn report(kind : i32, msg : *const c_char);

}

/// PostgreSQL raw byte array (bytea). Allows the user to write functions which
/// take Bytea as arguments (mapping to a bytea field at the SQL definition).
/// This structure just wraps a palloc-allocated pointer, so returning it from
/// functions is the same as returning *const varlena.
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Bytea(*const varlena);

impl Bytea {

    /// Allocates a buffer without initializing its contents. You can copy
    /// data into the buffer with:
    ///
    /// ```rust
    /// let mut b = Bytea::palloc(5);
    /// b.as_mut().copy_from_slice(&[0u8, 0u8, 0u8, 1u8, 1u8]);
    /// ```
    ///
    /// Make sure the passed slice has the exact same size that was allocated,
    /// at the penalty of Rust throwing a panic (if this panic is uncaught via
    /// catch_unwind, the server might crash and cut the connection). Use Bytea::from
    /// to allocate exactly the ammount of data you will need directly from a &[u8] or Vec<u8>.
    pub fn palloc(sz : usize) -> Self {
        unsafe {
            let vl_ptr : *const varlena = palloc_varlena(sz);
            Bytea(vl_ptr)
        }
    }

    /// Copies the content of data into a new buffer allocated via palloc
    pub fn from(data : &[u8]) -> Self {
        let mut b = Self::palloc(data.len());
        b.as_mut().copy_from_slice(data);
        b
    }

    /// Returns a &str from the buffer iff it represents valid UTF8
    pub fn as_str(&self) -> Option<&str> {
        std::str::from_utf8(self.as_ref()).ok()
    }

    /// Returns a &mut str from the buffer iff it represents valid UTF8
    pub fn as_str_mut(&mut self) -> Option<&mut str> {
        std::str::from_utf8_mut(self.as_mut()).ok()
    }

}

/// PostgreSQL text type. Just wraps a bytea, but adds the guarantee that the underlying
/// data is valid UTF-8. Implements AsRef<[str]> (while bytea does not). The VarChar type
/// and BpChar type are aliases to this structure. Text, unlike Bytea, cannot be allocated directly
/// via palloc, because we have to guarantee the data handed by Postgre is a buffer erased to UTF-8.
/// To acquire a text, allocate a generic buffer via let b = Bytea::palloc(n), then write a valid UTF-8 to the buffer
/// via b.as_mut().copy_from_slice(&str.as_bytes()); Then wrap the result from the fallible conversion via
/// let txt = b.try_into().unwrap();
#[derive(Clone, Copy, Debug)]
#[repr(C)]
pub struct Text(*const varlena);

impl Text {

    /// Allocates a buffer and copies the slice contents into it.
    pub fn from(content : &str) -> Self {
        let mut txt_bytes = Bytea::palloc(content.as_bytes().len());
        txt_bytes.as_mut().copy_from_slice(content.as_bytes());
        txt_bytes.try_into().unwrap()
    }

}

impl convert::TryInto<Text> for Bytea {

    type Error = ();

    fn try_into(self) -> Result<Text, ()> {
        if self.as_str().is_some() {
            Ok(Text(self.0))
        } else {
            Err(())
        }
    }
}

impl fmt::Display for Text {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_ref())
    }
}

pub type BpChar = Text;

pub type VarChar = Text;

// ABI-compatible struct with ByteSlice from pg_helper.c
#[repr(C)]
struct ByteSlice  {
    data : *const u8,
    len : usize
}

impl From<Vec<u8>> for ByteSlice {
    fn from(v : Vec<u8>) -> Self {
        let (data, _, cap) = unsafe{ v.into_raw_parts() };
        ByteSlice{ data, len : cap }
    }
}

fn bytes_to_slice<'a>(bytes : *const varlena) -> &'a [u8] {
    unsafe{ slice::from_raw_parts(bytes_ptr(bytes) as *mut _, bytes_len(bytes)) }
}

fn bytes_to_slice_mut<'a>(bytes : *mut varlena) -> &'a mut [u8] {
    unsafe{ slice::from_raw_parts_mut(bytes_ptr(bytes) as *mut _, bytes_len(bytes)) }
}

fn utf8_to_str<'a>(bytes : *const varlena) -> &'a str {
    unsafe{ std::str::from_utf8(bytes_to_slice(bytes as *const varlena)).unwrap() }
}

fn utf8_to_str_mut<'a>(bytes : *mut varlena) -> &'a mut str {
    unsafe{ std::str::from_utf8_mut(bytes_to_slice_mut(bytes)).unwrap() }
}

impl AsRef<[u8]> for Bytea {

    fn as_ref(&self) -> &[u8] {
        bytes_to_slice(self.0 as *const _)
    }
}

impl AsMut<[u8]> for Bytea {

    fn as_mut(&mut self) -> &mut [u8] {
        bytes_to_slice_mut(self.0 as *mut _)
    }
}

impl AsRef<str> for Text {

    fn as_ref(&self) -> &str {
        utf8_to_str(self.0 as *const _)
    }
}

impl AsMut<str> for Text {

    fn as_mut(&mut self) -> &mut str {
        utf8_to_str_mut(self.0 as *mut _)
    }
}

impl From<String> for Text {
    fn from(s : String) -> Self {
        let v : Vec<u8> = s.into();
        let b : Bytea = v.into();
        let txt : Text = b.try_into().unwrap();
        txt
    }
}

impl From<Vec<u8>> for Bytea {
    fn from(v : Vec<u8>) -> Self {
        unsafe {
            let vl_ptr : *const varlena = copy_bytes_to_pg(v);
            Self(vl_ptr)
        }
    }
}

/// Copies data from s into a buffer allocated via palloc, returning the
/// newly-allocated data pointer.
fn copy_bytes_to_pg(data_vec : Vec<u8>) -> *const varlena {
    // Recover points to original data and forget to clear it for now
    let (data, len, cap) = data_vec.into_raw_parts();
    let bs = ByteSlice{ data, len };

    unsafe {
        // Space is allocated via palloc here and data is copied into it
        let bytes_ptr = copy_to_pg(bs);

        // Re-build original content so Rust-allocated data can be dropped
        let _ = Vec::from_raw_parts(data, len, cap);

        bytes_ptr
    }
}

#[test]
fn test() {
    let txt = "Hello";
    //let bytes = Bytea::palloc(txt.as_bytes().len());
    println!("{:?}", txt.as_bytes().len());
}

/* Move this to a test sub-crate

create function return_text() returns text as
    '$libdir/libbayes.so', 'return_text'
language c strict;

create function alloc_text() returns text as
    '$libdir/libbayes.so', 'alloc_text'
language c strict;

create function text_len() returns integer as
    '$libdir/libbayes.so', 'text_len'
language c strict;

create function raise_err() returns integer as
    '$libdir/libbayes.so', 'raise_err'
language c strict;

#[no_mangle]
pub extern "C" fn alloc_text() -> Text {
    let hello = "hello";
    let mut txt_bytes = Bytea::palloc(hello.as_bytes().len());
    txt_bytes.as_mut().copy_from_slice(hello.as_bytes());
    txt_bytes.try_into().unwrap()
}

#[no_mangle]
pub extern "C" fn return_text() -> Text {
    let hello = "hello";
    let mut txt_bytes = Bytea::palloc(hello.as_bytes().len());
    txt_bytes.as_mut().copy_from_slice(hello.as_bytes());
    txt_bytes.try_into().unwrap()
}

#[no_mangle]
pub extern "C" fn text_len() -> i32 {
    let hello = "hello";
    let mut txt_bytes = Bytea::palloc(hello.as_bytes().len());
    txt_bytes.as_mut().len() as i32
}

#[no_mangle]
pub extern "C" fn raise_err() -> i32 {
    log::Error::raise("Some random error")
}
*/

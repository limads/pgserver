#![feature(vec_into_raw_parts)]

use std::slice;
use std::os::raw::c_char;
use std::fmt;
use std::convert::{self, TryInto};
use std::mem;

// split into error submodule
// -- Bindgen-generated code to reproduce a small subset of the PG types here.

#[repr(C)]
#[derive(Default)]
struct __IncompleteArrayField<T>(::std::marker::PhantomData<T>, [T; 0]);
impl<T> __IncompleteArrayField<T> {
    #[inline]
    pub const fn new() -> Self {
        __IncompleteArrayField(::std::marker::PhantomData, [])
    }
    #[inline]
    pub unsafe fn as_ptr(&self) -> *const T {
        ::std::mem::transmute(self)
    }
    #[inline]
    pub unsafe fn as_mut_ptr(&mut self) -> *mut T {
        ::std::mem::transmute(self)
    }
    #[inline]
    pub unsafe fn as_slice(&self, len: usize) -> &[T] {
        ::std::slice::from_raw_parts(self.as_ptr(), len)
    }
    #[inline]
    pub unsafe fn as_mut_slice(&mut self, len: usize) -> &mut [T] {
        ::std::slice::from_raw_parts_mut(self.as_mut_ptr(), len)
    }
}
impl<T> ::std::fmt::Debug for __IncompleteArrayField<T> {
    fn fmt(&self, fmt: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
        fmt.write_str("__IncompleteArrayField")
    }
}
impl<T> ::std::clone::Clone for __IncompleteArrayField<T> {
    #[inline]
    fn clone(&self) -> Self {
        Self::new()
    }
}

#[repr(C)]
struct __BindgenUnionField<T>(::std::marker::PhantomData<T>);
impl<T> __BindgenUnionField<T> {
    #[inline]
    pub const fn new() -> Self {
        __BindgenUnionField(::std::marker::PhantomData)
    }
    #[inline]
    pub unsafe fn as_ref(&self) -> &T {
        ::std::mem::transmute(self)
    }
    #[inline]
    pub unsafe fn as_mut(&mut self) -> &mut T {
        ::std::mem::transmute(self)
    }
}

#[repr(C)]
#[derive(Debug, Clone)]
struct varlena {
    vl_len_: [::std::os::raw::c_char; 4usize],
    vl_dat: __IncompleteArrayField<::std::os::raw::c_char>,
}

// Available at link time from the pg_helper.c module.
// #[link(name = "pghelper", kind="static")]
extern "C" {

    fn read_from_pg(arg : *const varlena) -> ByteSlice;

    fn palloc_varlena(sz : usize) -> *const varlena;

    fn copy_to_pg(s : ByteSlice) -> *const varlena;

    fn bytes_ptr(t : *const varlena) -> *const u8;

    fn bytes_len(t : *const varlena) -> usize;

}

/// PostgreSQL raw byte array (bytea). Allows the user to write functions which
/// take Bytea as arguments (mapping to a bytea field at the SQL definition).
/// This structure just wraps a palloc-allocated pointer, so returning it from
/// functions is the same as returning *const varlena.
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

/*impl Write for Bytea {

    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s = self.as_mut();
        io::Error::new()
    }
}*/

/// PostgreSQL text type. Just wraps a bytea, but adds the guarantee that the underlying
/// data is valid UTF-8. Implements AsRef<[str]> (while bytea does not). The VarChar type
/// and BpChar type are aliases to this structure. Text, unlike Bytea, cannot be allocated directly
/// via palloc, because we have to guarantee the data handed by Postgre is a buffer erased to UTF-8.
/// To acquire a text, allocate a generic buffer via let b = Bytea::palloc(n), then write a valid UTF-8 to the buffer
/// via b.as_mut().copy_from_slice(&str.as_bytes()); Then wrap the result from the fallible conversion via
/// let txt = b.try_into().unwrap();
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

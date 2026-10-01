use std::{
    alloc::{alloc, dealloc, handle_alloc_error, Layout},
    collections::BTreeMap,
    mem::{align_of, size_of_val},
    path::PathBuf,
};

/// Allocate a buffer for the host to write into (e.g. for `read_file_v2`
/// and `get_attrset`). The host returns the pointer to the guest, which
/// must free it with the same size and alignment (e.g. by reconstructing a
/// `Vec<u8>` if `align` is 1).
#[no_mangle]
pub extern "C" fn nix_wasm_alloc(size: usize, align: usize) -> *mut u8 {
    let layout = Layout::from_size_align(size, align).expect("invalid allocation request");
    if size == 0 {
        // Zero-sized allocations are not allowed, so return a dangling
        // (but aligned) pointer.
        return align as *mut u8;
    }
    let ptr = unsafe { alloc(layout) };
    if ptr.is_null() {
        handle_alloc_error(layout);
    }
    ptr
}

#[no_mangle]
pub extern "C" fn nix_wasm_init_v1() {
    std::panic::set_hook(Box::new(|panic_info| {
        panic(&format!("{}", panic_info));
    }));
}

pub fn panic(s: &str) -> ! {
    extern "C" {
        fn panic(ptr: *const u8, len: usize) -> !;
    }
    unsafe { panic(s.as_ptr(), s.len()) }
}

#[macro_export]
macro_rules! warn {
    ( $( $t:tt )* ) => {
        {
            extern "C" {
                fn warn(ptr: *const u8, len: usize);
            }
            unsafe {
                let s = format!( $( $t )* );
                warn(s.as_ptr(), s.len());
            }
        }
    };
}

// FIXME: use externref for Values?
#[repr(transparent)]
#[derive(Clone, Debug, Copy)]
pub struct Value(ValueId);

type ValueId = u32;

pub fn wasi_arg() -> Value {
    let arg = std::env::args()
        .nth(1)
        .unwrap_or_else(|| panic("missing WASI argument"));
    let value_id = arg
        .parse::<ValueId>()
        .unwrap_or_else(|err| panic(&format!("invalid WASI argument '{arg}': {err}")));
    Value::from_raw(value_id)
}

#[repr(C)]
pub enum Type {
    Int = 1,
    Float = 2,
    Bool = 3,
    String = 4,
    Path = 5,
    Null = 6,
    Attrs = 7,
    List = 8,
    Function = 9,
}

impl Value {
    pub fn from_raw(value: ValueId) -> Value {
        Value(value)
    }

    pub fn get_type(&self) -> Type {
        extern "C" {
            fn get_type(value: ValueId) -> Type;
        }
        unsafe { get_type(self.0) }
    }

    pub fn make_int(n: i64) -> Value {
        extern "C" {
            fn make_int(value: i64) -> Value;
        }
        unsafe { make_int(n) }
    }

    pub fn get_int(&self) -> i64 {
        extern "C" {
            fn get_int(value: ValueId) -> i64;
        }
        unsafe { get_int(self.0) }
    }

    pub fn make_float(f: f64) -> Value {
        extern "C" {
            fn make_float(value: f64) -> Value;
        }
        unsafe { make_float(f) }
    }

    pub fn get_float(&self) -> f64 {
        extern "C" {
            fn get_float(value: ValueId) -> f64;
        }
        unsafe { get_float(self.0) }
    }

    pub fn make_string(s: &str) -> Value {
        extern "C" {
            fn make_string(ptr: *const u8, len: usize) -> Value;
        }
        unsafe { make_string(s.as_ptr(), s.len()) }
    }

    pub fn get_string(&self) -> String {
        extern "C" {
            fn copy_string(value: ValueId, ptr: *mut u8, max_len: usize) -> usize;
        }
        unsafe {
            // Optimistically call with a small buffer on the stack.
            let mut buf = [0; 256];
            let len = copy_string(self.0, buf.as_mut_ptr(), buf.len());
            if len > buf.len() {
                // If it didn't fit, allocate a buffer of the right size.
                let mut buf = vec![0; len];
                let len2 = copy_string(self.0, buf.as_mut_ptr(), buf.len());
                assert!(len2 == len);
                String::from_utf8(buf).expect("Nix string should be UTF-8.")
            } else {
                String::from_utf8(buf[0..len].to_vec()).expect("Nix string should be UTF-8.")
            }
        }
    }

    /// Create a new path value relative to this one, within the same source tree.
    pub fn make_path(&self, rel: &str) -> Value {
        extern "C" {
            fn make_path(base: ValueId, ptr: *const u8, len: usize) -> Value;
        }
        unsafe { make_path(self.0, rel.as_ptr(), rel.len()) }
    }

    pub fn get_path(&self) -> PathBuf {
        extern "C" {
            fn copy_path(value: ValueId, ptr: *mut u8, max_len: usize) -> usize;
        }
        unsafe {
            // Optimistically call with a small buffer on the stack.
            let mut buf = [0; 256];
            let len = copy_path(self.0, buf.as_mut_ptr(), buf.len());
            if len > buf.len() {
                // If it didn't fit, allocate a buffer of the right size.
                let mut buf = vec![0; len];
                let len2 = copy_path(self.0, buf.as_mut_ptr(), buf.len());
                assert!(len2 == len);
                String::from_utf8(buf)
                    .expect("Nix path should be UTF-8.")
                    .into()
            } else {
                String::from_utf8(buf[0..len].to_vec())
                    .expect("Nix path should be UTF-8.")
                    .into()
            }
        }
    }

    pub fn make_bool(b: bool) -> Value {
        extern "C" {
            fn make_bool(b: bool) -> Value;
        }
        unsafe { make_bool(b) }
    }

    pub fn get_bool(&self) -> bool {
        extern "C" {
            fn get_bool(value: ValueId) -> bool;
        }
        unsafe { get_bool(self.0) }
    }

    pub fn make_null() -> Value {
        extern "C" {
            fn make_null() -> Value;
        }
        unsafe { make_null() }
    }

    pub fn make_list(list: &[Value]) -> Value {
        extern "C" {
            fn make_list(ptr: *const Value, len: usize) -> Value;
        }
        unsafe { make_list(list.as_ptr(), list.len()) }
    }

    pub fn get_list(&self) -> Vec<Value> {
        extern "C" {
            fn copy_list(value: ValueId, ptr: *mut Value, max_len: usize) -> usize;
        }
        unsafe {
            // Optimistically call with a small buffer on the stack.
            let mut buf = [Value(0); 64];
            let len = copy_list(self.0, buf.as_mut_ptr(), buf.len());
            if len > buf.len() {
                // If it didn't fit, allocate a buffer of the right size.
                let mut buf = vec![Value(0); len];
                let len2 = copy_list(self.0, buf.as_mut_ptr(), buf.len());
                assert!(len2 == len);
                buf
            } else {
                buf[0..len].to_vec()
            }
        }
    }

    pub fn make_attrset(attrs: &[(&str, Value)]) -> Value {
        extern "C" {
            #[allow(improper_ctypes)]
            fn make_attrset(ptr: *const (&str, Value), len: usize) -> Value;
        }
        unsafe { make_attrset(attrs.as_ptr(), attrs.len()) }
    }

    /// Decode the buffer written by the `get_attrset` host function: the
    /// number of attributes `n`, then `n` value IDs, then `n` null-terminated
    /// attribute names. The buffer is aligned to 4 bytes.
    fn decode_attrset(buf: &[u8]) -> BTreeMap<String, Value> {
        let (prefix, ints, _) = unsafe { buf.align_to::<ValueId>() };
        assert!(prefix.is_empty());
        let n = ints[0] as usize;
        let values = &ints[1..=n];
        let names = &buf[size_of_val(&ints[0..=n])..];
        values
            .iter()
            .zip(names.split(|b| *b == 0))
            .map(|(value, name)| {
                (
                    String::from_utf8(name.to_vec()).expect("Nix attribute name should be UTF-8."),
                    Value(*value),
                )
            })
            .collect()
    }

    /// Get the attributes of an attrset. The `get_attrset` host function
    /// returns all value IDs and attribute names in a single buffer. It uses
    /// the buffer we pass if that's large enough, and otherwise allocates one
    /// in the guest through `nix_wasm_alloc`. It returns the pointer of the
    /// buffer it used in the low 32 bits and the number of bytes written in
    /// the high 32 bits.
    pub fn get_attrset(&self) -> BTreeMap<String, Value> {
        extern "C" {
            fn get_attrset(value: ValueId, ptr: *mut u8, len: usize) -> u64;
        }
        unsafe {
            // Optimistically call with a small buffer on the stack. It has
            // to be aligned for `ValueId`s.
            let mut buf: [ValueId; 256] = [0; 256];
            let buf_ptr = buf.as_mut_ptr() as *mut u8;
            let packed = get_attrset(self.0, buf_ptr, size_of_val(&buf));
            let ptr = packed as u32 as *mut u8;
            let len = (packed >> 32) as usize;
            let res = Self::decode_attrset(std::slice::from_raw_parts(ptr, len));
            if ptr != buf_ptr {
                // It didn't fit, so the host allocated a buffer of the right size.
                dealloc(
                    ptr,
                    Layout::from_size_align_unchecked(len, align_of::<ValueId>()),
                );
            }
            res
        }
    }

    pub fn get_attr(&self, attr_name: &str) -> Option<Value> {
        extern "C" {
            fn get_attr(value: ValueId, ptr: *const u8, len: usize) -> ValueId;
        }
        let value_id = unsafe { get_attr(self.0, attr_name.as_ptr(), attr_name.len()) };
        if value_id == 0 {
            None
        } else {
            Some(Value(value_id))
        }
    }

    pub fn call(&self, args: &[Value]) -> Value {
        extern "C" {
            fn call_function(fun: ValueId, ptr: *const Value, len: usize) -> Value;
        }
        unsafe { call_function(self.0, args.as_ptr(), args.len()) }
    }

    pub fn lazy_call(&self, args: &[Value]) -> Value {
        extern "C" {
            fn make_app(fun: ValueId, ptr: *const Value, len: usize) -> Value;
        }
        unsafe { make_app(self.0, args.as_ptr(), args.len()) }
    }

    /// Read a file. The `read_file_v2` host function writes into a buffer it
    /// allocates in the guest through `nix_wasm_alloc`, so that the file is
    /// read and copied only once. It returns the buffer pointer in the low
    /// 32 bits and the length in the high 32 bits, since a function
    /// returning multiple Wasm values cannot be imported from Rust.
    pub fn read_file(&self) -> Vec<u8> {
        extern "C" {
            fn read_file_v2(value: ValueId) -> u64;
        }
        unsafe {
            let packed = read_file_v2(self.0);
            let ptr = packed as u32 as *mut u8;
            let len = (packed >> 32) as usize;
            Vec::from_raw_parts(ptr, len, len)
        }
    }

    pub fn return_to_nix(&self) -> ! {
        extern "C" {
            fn return_to_nix(value: ValueId) -> !;
        }
        unsafe { return_to_nix(self.0) }
    }
}

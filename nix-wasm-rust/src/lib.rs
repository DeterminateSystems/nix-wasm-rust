use std::collections::BTreeMap;

pub fn warn(s: &str) {
    extern "C" {
        fn warn(ptr: *const u8, len: usize);
    }
    unsafe {
        warn(s.as_ptr(), s.len());
    }
}

// FIXME: use externref for Values?
#[repr(transparent)]
#[derive(Clone, Debug)]
pub struct Value(ValueId);

type ValueId = u32;

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
        let len = self.get_string_length();
        let buf: Vec<u8> = vec![0; len];
        extern "C" {
            fn copy_string(value: ValueId, ptr: *const u8, len: usize);
        }
        unsafe {
            copy_string(self.0, buf.as_ptr(), len);
        }
        String::from_utf8(buf).expect("Nix string should be UTF-8.")
    }

    pub fn get_string_length(&self) -> usize {
        extern "C" {
            fn get_string_length(value: ValueId) -> usize;
        }
        unsafe { get_string_length(self.0) }
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
        let len = self.get_list_length();
        let res: Vec<Value> = vec![Value(0); len];
        extern "C" {
            fn copy_list(value: ValueId, ptr: u32, len: usize);
        }
        unsafe {
            if len > 0 {
                copy_list(self.0, res.as_ptr() as u32, len);
            }
        }
        res
    }

    pub fn get_list_length(&self) -> usize {
        extern "C" {
            fn get_list_length(value: ValueId) -> usize;
        }
        unsafe { get_list_length(self.0) }
    }

    pub fn make_attrset(attrs: &[(&str, Value)]) -> Value {
        extern "C" {
            fn make_attrset(ptr: u32, len: usize) -> Value;
        }
        unsafe { make_attrset(attrs.as_ptr() as u32, attrs.len()) }
    }

    pub fn get_attrset(&self) -> BTreeMap<String, Value> {
        extern "C" {
            fn get_attrset_length(value: ValueId) -> usize;
            fn copy_attrset(value: ValueId, ptr: u32, len: usize);
            fn copy_attrname(value: ValueId, attr_idx: usize, ptr: u32, len: usize);
        }
        let len = unsafe { get_attrset_length(self.0) };
        let attrs: Vec<(ValueId, usize)> = vec![(0, 0); len];
        if len > 0 {
            unsafe {
                copy_attrset(self.0, attrs.as_ptr() as u32, len);
            }
        }
        let mut res = BTreeMap::new();
        for (attr_idx, (value, attr_len)) in attrs.iter().enumerate() {
            let buf: Vec<u8> = vec![0; *attr_len];
            unsafe {
                copy_attrname(self.0, attr_idx, buf.as_ptr() as u32, *attr_len);
            }
            res.insert(
                String::from_utf8(buf).expect("Nix attribute name should be UTF-8."),
                Value(*value),
            );
        }
        res
    }
}

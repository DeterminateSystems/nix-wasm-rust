use nix_wasm_rust::Value;
use std::sync::atomic::{AtomicI64, Ordering};

#[no_mangle]
pub extern "C" fn range(args: Value) -> Value {
    let start = args.get_attr("start").map(|v| v.get_int()).unwrap_or(0);
    let end = args.get_attr("end").map(|v| v.get_int()).unwrap_or(10);

    let mut list = vec![];

    for i in start..=end {
        list.push(Value::make_int(i));
    }

    Value::make_list(&list)
}

#[no_mangle]
pub extern "C" fn strictMap(args: Value) -> Value {
    let fun = args.get_attr("fun").expect("missing 'fun' argument");

    let list = args
        .get_attr("list")
        .expect("missing 'list' argument")
        .get_list();

    let mut res = vec![];

    for v in list {
        res.push(fun.call(&[v]));
    }

    Value::make_list(&res)
}

#[no_mangle]
pub extern "C" fn lazyMap(args: Value) -> Value {
    let fun = args.get_attr("fun").expect("missing 'fun' argument");

    let list = args
        .get_attr("list")
        .expect("missing 'list' argument")
        .get_list();

    let mut res = vec![];

    for v in list {
        res.push(fun.lazy_call(&[v]));
    }

    Value::make_list(&res)
}

#[no_mangle]
pub extern "C" fn sum(arg: Value) -> Value {
    Value::make_int(arg.get_list().iter().map(|v| v.get_int()).sum())
}

#[no_mangle]
pub extern "C" fn double(arg: Value) -> Value {
    Value::make_int(arg.get_int() * 2)
}

#[no_mangle]
pub extern "C" fn counter(_arg: Value) -> Value {
    static COUNTER: AtomicI64 = AtomicI64::new(1);
    let current = COUNTER.fetch_add(1, Ordering::SeqCst);
    Value::make_int(current)
}

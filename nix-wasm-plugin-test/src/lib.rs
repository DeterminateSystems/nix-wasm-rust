use nix_wasm_rust::Value;

#[no_mangle]
pub extern "C" fn range(arg: Value) -> Value {
    let args = arg.get_attrset();

    let start = args.get("start").map(|v| v.get_int()).unwrap_or(0);
    let end = args.get("end").map(|v| v.get_int()).unwrap_or(10);

    let mut list = vec![];

    for i in start..=end {
        list.push(Value::make_int(i));
    }

    Value::make_list(&list)
}

#[no_mangle]
pub extern "C" fn map(arg: Value) -> Value {
    let args = arg.get_attrset();

    let fun = args.get("fun").expect("missing 'fun' argument");

    let list = args
        .get("list")
        .expect("missing 'list' argument")
        .get_list();

    let mut res = vec![];

    for v in list {
        res.push(fun.call(&[v]));
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

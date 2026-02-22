use nix_wasm_rust::{nix_wasm_init_v1, warn, wasi_arg, Value};
use rquickjs::{Array, Context, Object, Runtime, Value as JsValue};
use std::string::String as StdString;

fn fail(context: &str, err: impl std::fmt::Display) -> ! {
    warn!("quickjs {context} failed: {err}");
    panic!("quickjs {context} failed: {err}");
}

fn js_value_to_nix(value: JsValue) -> Value {
    if value.is_null() || value.is_undefined() {
        return Value::make_null();
    }
    if let Some(b) = value.as_bool() {
        return Value::make_bool(b);
    }
    if let Some(i) = value.as_int() {
        return Value::make_int(i as i64);
    }
    if let Some(f) = value.as_float() {
        return Value::make_float(f);
    }
    if let Some(js_str) = value.as_string() {
        let s = js_str.to_string().unwrap_or_else(|err| fail("string conversion", err));
        return Value::make_string(&s);
    }
    if value.is_array() {
        let array: Array = value
            .clone()
            .into_array()
            .unwrap_or_else(|| fail("array conversion", "value is not an array"));
        let mut items = Vec::new();
        for entry in array.into_iter() {
            let entry = entry.unwrap_or_else(|err| fail("array iteration", err));
            items.push(js_value_to_nix(entry));
        }
        return Value::make_list(&items);
    }
    if value.is_object() {
        let object: Object = value
            .into_object()
            .unwrap_or_else(|| fail("object conversion", "value is not an object"));
        let mut entries: Vec<(StdString, Value)> = Vec::new();
        for entry in object.props::<StdString, JsValue>() {
            let (key, value) = entry.unwrap_or_else(|err| fail("object iteration", err));
            entries.push((key, js_value_to_nix(value)));
        }
        let attrs: Vec<(&str, Value)> = entries
            .iter()
            .map(|(key, value)| (key.as_str(), *value))
            .collect();
        return Value::make_attrset(&attrs);
    }

    warn!("quickjs value type not supported: {:?}", value.type_of());
    panic!("quickjs value type not supported: {:?}", value.type_of());
}

fn eval_impl(arg: Value) -> Value {
    let code = arg.get_string();

    let runtime = Runtime::new().unwrap_or_else(|err| fail("runtime init", err));
    let context = Context::full(&runtime).unwrap_or_else(|err| fail("context init", err));

    context.with(|ctx| {
        let value: JsValue = ctx.eval(code).unwrap_or_else(|err| fail("eval", err));
        js_value_to_nix(value)
    })
}

#[no_mangle]
pub extern "C" fn eval(arg: Value) -> Value {
    nix_wasm_init_v1();
    eval_impl(arg)
}

#[no_mangle]
pub extern "C" fn _start() {
    nix_wasm_init_v1();
    let result = eval_impl(wasi_arg());
    result.return_to_nix();
}

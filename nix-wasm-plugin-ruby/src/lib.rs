use artichoke_backend::prelude::{Eval, Value as ArtichokeValue};
use nix_wasm_rust::{nix_wasm_init_v1, warn, wasi_arg, Value};

fn eval_impl(arg: Value) -> Value {
    let code = arg.get_string();
    let mut interp = artichoke_backend::interpreter().unwrap_or_else(|err| {
        warn!("ruby interp init failed: {err}");
        panic!("ruby interp init failed: {err}");
    });

    let result = interp.eval(code.as_bytes()).unwrap_or_else(|err| {
        warn!("ruby eval failed: {err}");
        panic!("ruby eval failed: {err}");
    });

    let rendered = result
        .try_convert_into_mut::<String>(&mut interp)
        .unwrap_or_else(|err| {
            warn!("ruby to string failed: {err}");
            panic!("ruby to string failed: {err}");
        });

    Value::make_string(&rendered)
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

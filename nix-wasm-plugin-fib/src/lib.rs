use nix_wasm_rust::{warn, Value};

#[no_mangle]
pub extern "C" fn fib(arg: Value) -> Value {
    warn!("greetings from Wasm!");

    fn fib2(n: i64) -> i64 {
        if n < 2 {
            1
        } else {
            fib2(n - 1) + fib2(n - 2)
        }
    }

    Value::make_int(fib2(arg.get_int()))
}

use nix_wasm_plugin_fib::fib;
use nix_wasm_rust::Value;
use std::env;

fn main() {
    eprintln!("Greetings from WASI!");
    eprintln!("Environment size: {}", env::vars().count());
    eprintln!(
        "System time: {}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs()
    );
    eprintln!(
        "Current directory: {}",
        std::env::current_dir().unwrap().display()
    );
    eprintln!(
        "Number of files in /: {}",
        std::fs::read_dir("/").map(|x| x.count()).unwrap_or(0)
    );

    /* The argument value ID is passed via argv[1]. */
    let args: Vec<String> = env::args().collect();
    let arg = Value::from_id(
        args[1]
            .parse::<u32>()
            .expect("argv[1] should be a valid ValueId"),
    );

    /* Do the computation. */
    let result = fib(arg);

    /* Return the value to the host. This also ends execution. */
    result.return_to_nix();
}

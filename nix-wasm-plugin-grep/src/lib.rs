use nix_wasm_rust::Value;

#[no_mangle]
pub extern "C" fn grep(args: Value) -> Value {
    let builtins = args
        .get_attr("builtins")
        .expect("missing 'builtins' argument");
    let path = args.get_attr("path").expect("missing 'path' argument");
    let pattern = args
        .get_attr("pattern")
        .expect("missing 'pattern' argument")
        .get_string();

    let read_dir = builtins.get_attr("readDir").unwrap();

    let mut matches = vec![];
    recurse(&read_dir, &pattern, &path, &mut matches);

    Value::make_list(&matches)
}

fn recurse(read_dir: &Value, pattern: &str, path_val: &Value, matches: &mut Vec<Value>) {
    for (name, file_type) in read_dir.call(&[*path_val]).get_attrset() {
        let child = path_val.make_path(&name);
        let file_type = file_type.get_string();
        match file_type.as_str() {
            "regular" => {
                // FIXME: support searching in files that are not UTF-8.
                if let Ok(contents) = String::from_utf8(child.read_file()) {
                    if contents.contains(pattern) {
                        matches.push(child);
                    }
                }
            }
            "directory" => {
                recurse(read_dir, pattern, &child, matches);
            }
            _ => {}
        }
    }
}

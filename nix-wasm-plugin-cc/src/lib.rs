use nix_wasm_rust::{warn, Value};
use std::collections::{HashMap, HashSet};

struct File {
    path: String,
    includes: Vec<String>,
}

#[no_mangle]
pub extern "C" fn cc(args: Value) -> Value {
    let builtins = args
        .get_attr("builtins")
        .expect("missing 'builtins' argument");
    let path = args.get_attr("path").expect("missing 'path' argument");

    let read_dir = builtins.get_attr("readDir").unwrap();

    // First pass: scan all .cc and .hh files, recording their direct includes.
    let mut cc_files: Vec<Value> = vec![];
    let mut all_files: HashMap<Value, File> = HashMap::new();
    scan_files(&read_dir, &path, "", &mut cc_files, &mut all_files);

    // Build a suffix map for efficient include resolution.
    // For a file "foo/bar/xyzzy/util.hh", this inserts:
    //   "util.hh" -> "foo/bar/xyzzy/util.hh"
    //   "xyzzy/util.hh" -> "foo/bar/xyzzy/util.hh"
    //   "bar/xyzzy/util.hh" -> "foo/bar/xyzzy/util.hh"
    //   "foo/bar/xyzzy/util.hh" -> "foo/bar/xyzzy/util.hh"
    let mut suffix_map: HashMap<String, Value> = HashMap::new();
    for (value, file) in &all_files {
        let rel_path = &file.path;
        let parts: Vec<&str> = rel_path.split('/').collect();
        for i in 0..parts.len() {
            let suffix = parts[i..].join("/");
            suffix_map.entry(suffix).or_insert_with(|| *value);
        }
    }

    // Second pass: for each .cc file, compute the transitive closure of includes.
    let mut results = vec![];
    for cc_file_val in &cc_files {
        //warn!("processing {path}...", path = all_files[cc_file_val].path);
        let mut all_includes: HashMap<String, Value> = HashMap::new();
        let mut visited = HashSet::new();
        collect_transitive_includes(
            *cc_file_val,
            &all_files,
            &suffix_map,
            &mut all_includes,
            &mut visited,
        );

        let mut sorted_includes: Vec<_> = all_includes.iter().collect();
        sorted_includes.sort_by_key(|(include, _)| *include);
        let include_values: Vec<Value> = sorted_includes
            .iter()
            .map(|(include, path_val)| {
                Value::make_attrset(&[
                    ("include", Value::make_string(include)),
                    ("path", **path_val),
                ])
            })
            .collect();
        let entry = Value::make_attrset(&[
            ("file", *cc_file_val),
            ("includes", Value::make_list(&include_values)),
        ]);
        results.push(entry);
    }

    Value::make_list(&results)
}

fn scan_files(
    read_dir: &Value,
    path_val: &Value,
    prefix: &str,
    cc_files: &mut Vec<Value>,
    all_files: &mut HashMap<Value, File>,
) {
    for (name, file_type) in read_dir.call(&[*path_val]).get_attrset() {
        let child = path_val.make_path(&name);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = file_type.get_string();
        match file_type.as_str() {
            "regular" => {
                if name.ends_with(".cc") || name.ends_with(".hh") {
                    let contents = child.read_file();
                    let includes = extract_includes(&contents);
                    if name.ends_with(".cc") {
                        cc_files.push(child);
                    }
                    all_files.insert(child, File { path, includes });
                }
            }
            "directory" => {
                scan_files(read_dir, &child, &path, cc_files, all_files);
            }
            _ => {}
        }
    }
}

fn collect_transitive_includes(
    file: Value,
    all_files: &HashMap<Value, File>,
    suffix_map: &HashMap<String, Value>,
    all_includes: &mut HashMap<String, Value>,
    visited: &mut HashSet<Value>,
) {
    if !visited.insert(file) {
        return;
    }

    let file = &all_files[&file];

    for inc in &file.includes {
        if let Some(resolved) = suffix_map.get(inc) {
            all_includes.entry(inc.clone()).or_insert(*resolved);
            collect_transitive_includes(*resolved, all_files, suffix_map, all_includes, visited);
        } else {
            warn!("{file}: included file not found: {inc}", file = file.path);
        }
    }
}

fn extract_includes(contents: &[u8]) -> Vec<String> {
    let Ok(text) = std::str::from_utf8(contents) else {
        return vec![];
    };
    let mut includes = vec![];
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix('#') {
            let rest = rest.trim_start();
            let Some(rest) = rest.strip_prefix("include") else {
                continue;
            };
            let rest = rest.trim();
            if let Some(path) = rest.strip_prefix('"') {
                if let Some(path) = path.strip_suffix('"') {
                    includes.push(path.to_string());
                }
            }
        }
    }
    includes
}

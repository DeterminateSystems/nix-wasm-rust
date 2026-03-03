use nix_wasm_rust::{warn, Value};
use std::collections::{HashMap, HashSet};

struct FileInfo {
    path: String,
    sys_includes: Vec<String>,
    user_includes: Vec<String>,
}

#[no_mangle]
pub extern "C" fn cc(args: Value) -> Value {
    let builtins = args
        .get_attr("builtins")
        .expect("missing 'builtins' argument");
    let dirs = args.get_attr("dirs").expect("missing 'dirs' argument");

    let read_dir = builtins.get_attr("readDir").unwrap();

    // First pass: scan all .cc and .hh files, recording their direct includes.
    let mut all_files: HashMap<Value, FileInfo> = HashMap::new();
    for entry in dirs.get_list() {
        let root = entry.get_attr("root").expect("missing 'root' attribute");
        let prefix = entry
            .get_attr("prefix")
            .expect("missing 'prefix' attribute")
            .get_string();
        scan_files(&read_dir, &root, &prefix, &mut all_files);
    }

    // Process explicit files: each key is a path (possibly with slashes),
    // the value is the file's path value.
    if let Some(files) = args.get_attr("files") {
        for (name, file_val) in files.get_attrset() {
            let contents = file_val.read_file();
            let file_info = extract_includes(name, &contents);
            all_files.insert(file_val, file_info);
        }
    }

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
    for (cc_file_val, cc_file) in all_files.iter() {
        if !cc_file.path.ends_with(".cc") {
            continue;
        }
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

        let include_attrs: Vec<(&str, Value)> = all_includes
            .values()
            .map(|path_val| {
                let inc_file = &all_files[path_val];
                (inc_file.path.as_str(), *path_val)
            })
            .collect();
        let entry = Value::make_attrset(&[
            ("src", *cc_file_val),
            ("path", Value::make_string(&cc_file.path)),
            ("includes", Value::make_attrset(&include_attrs)),
        ]);
        results.push(entry);
    }

    Value::make_list(&results)
}

fn scan_files(
    read_dir: &Value,
    path_val: &Value,
    prefix: &str,
    all_files: &mut HashMap<Value, FileInfo>,
) {
    for (name, file_type) in read_dir.call(&[*path_val]).get_attrset() {
        if name == "windows" {
            continue;
        } // FIXME: hack
        let child = path_val.make_path(&name);
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let file_type = file_type.get_string();
        match file_type.as_str() {
            "regular" => {
                if name.ends_with(".cc")
                    || name.ends_with(".hh")
                    || name.ends_with(".h")
                    || name.ends_with(".sb")
                    || name.ends_with(".md")
                {
                    let contents = child.read_file();
                    let file_info = extract_includes(path, &contents);
                    all_files.insert(child, file_info);
                }
            }
            "directory" => {
                scan_files(read_dir, &child, &path, all_files);
            }
            _ => {}
        }
    }
}

fn collect_transitive_includes(
    file: Value,
    all_files: &HashMap<Value, FileInfo>,
    suffix_map: &HashMap<String, Value>,
    all_includes: &mut HashMap<String, Value>,
    visited: &mut HashSet<Value>,
) {
    if !visited.insert(file) {
        return;
    }

    let file = &all_files[&file];

    for inc in &file.user_includes {
        if let Some(resolved) = suffix_map.get(inc) {
            all_includes.entry(inc.clone()).or_insert(*resolved);
            collect_transitive_includes(*resolved, all_files, suffix_map, all_includes, visited);
        } else {
            // FIXME: hack
            if !inc.contains("windows") {
                warn!("{file}: included file not found: {inc}", file = file.path);
            }
        }
    }
}

fn extract_includes(path: String, contents: &[u8]) -> FileInfo {
    let mut file_info = FileInfo {
        path,
        sys_includes: vec![],
        user_includes: vec![],
    };
    let Ok(text) = std::str::from_utf8(contents) else {
        return file_info;
    };
    // FIXME: process #ifdefs so we can skip #includes that don't apply
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
                    file_info.user_includes.push(path.to_string());
                }
            }
            if let Some(path) = rest.strip_prefix('<') {
                if let Some(path) = path.strip_suffix('>') {
                    file_info.sys_includes.push(path.to_string());
                }
            }
        }
    }
    file_info
}

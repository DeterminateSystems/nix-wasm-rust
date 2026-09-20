//! `#include` dependency scanner for using Nix as a C/C++ build system.
//!
//! See README.md for the argument and result schema of `getDeps`.

use nix_wasm_rust::{warn, Value};
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// An `#include` directive: the included path and whether it used angle brackets.
struct Include {
    path: String,
    angle: bool,
}

struct FileInfo {
    /// The Nix value (path or string) denoting this file.
    value: Value,
    includes: Vec<Include>,
}

/// Index of all known files, keyed by their path relative to the root namespace.
type Index = BTreeMap<String, FileInfo>;

#[no_mangle]
pub extern "C" fn getDeps(args: Value) -> Value {
    let builtins = args
        .get_attr("builtins")
        .expect("missing 'builtins' argument");
    let read_dir = builtins.get_attr("readDir").unwrap();

    // Build the file index by scanning the roots and adding explicit files.
    let mut index = Index::new();
    for entry in args
        .get_attr("roots")
        .expect("missing 'roots' argument")
        .get_list()
    {
        let root = entry.get_attr("root").expect("missing 'root' attribute");
        let prefix = entry
            .get_attr("prefix")
            .expect("missing 'prefix' attribute")
            .get_string();
        scan_files(&read_dir, &root, &prefix, &mut index);
    }

    if let Some(files) = args.get_attr("files") {
        for (name, value) in files.get_attrset() {
            let includes = extract_includes(&value.read_file());
            index.insert(normalize(&name), FileInfo { value, includes });
        }
    }

    let include_dirs: Vec<String> = args
        .get_attr("includeDirs")
        .map(|v| {
            v.get_list()
                .iter()
                .map(|d| normalize(&d.get_string()))
                .collect()
        })
        .unwrap_or_default();

    let sources: BTreeSet<String> = args
        .get_attr("sources")
        .expect("missing 'sources' argument")
        .get_list()
        .iter()
        .map(|s| normalize(&s.get_string()))
        .collect();

    let mut results = vec![];
    for source in &sources {
        let Some(file) = index.get(source) else {
            panic!("source file '{source}' not found in the scanned roots or explicit files");
        };

        let mut includes: BTreeMap<String, Value> = BTreeMap::new();
        let mut external: BTreeSet<String> = BTreeSet::new();
        let mut visited: HashSet<String> = HashSet::new();
        collect_transitive_includes(
            source,
            &index,
            &include_dirs,
            &mut includes,
            &mut external,
            &mut visited,
        );

        let include_attrs: Vec<(&str, Value)> = includes
            .iter()
            .map(|(path, value)| (path.as_str(), *value))
            .collect();
        let external_values: Vec<Value> = external.iter().map(|s| Value::make_string(s)).collect();

        results.push(Value::make_attrset(&[
            ("path", Value::make_string(source)),
            ("src", file.value),
            ("includes", Value::make_attrset(&include_attrs)),
            ("externalIncludes", Value::make_list(&external_values)),
        ]));
    }

    Value::make_list(&results)
}

fn is_source_file(name: &str) -> bool {
    [".cc", ".hh", ".h", ".sb", ".md"]
        .iter()
        .any(|ext| name.ends_with(ext))
}

fn scan_files(read_dir: &Value, dir: &Value, prefix: &str, index: &mut Index) {
    for (name, file_type) in read_dir.call(&[*dir]).get_attrset() {
        let child = dir.make_path(&name);
        let path = join(prefix, &name);
        match file_type.get_string().as_str() {
            "regular" => {
                if is_source_file(&name) {
                    let includes = extract_includes(&child.read_file());
                    index.insert(
                        path,
                        FileInfo {
                            value: child,
                            includes,
                        },
                    );
                }
            }
            "directory" => scan_files(read_dir, &child, &path, index),
            // Symlinks are ignored (e.g. `nix-meson-build-support` -> `../../nix-meson-build-support`).
            _ => {}
        }
    }
}

fn collect_transitive_includes(
    path: &str,
    index: &Index,
    include_dirs: &[String],
    includes: &mut BTreeMap<String, Value>,
    external: &mut BTreeSet<String>,
    visited: &mut HashSet<String>,
) {
    if !visited.insert(path.to_string()) {
        return;
    }

    let file = &index[path];

    for inc in &file.includes {
        match resolve(index, include_dirs, path, inc) {
            Some(resolved) => {
                includes.insert(resolved.clone(), index[&resolved].value);
                collect_transitive_includes(
                    &resolved,
                    index,
                    include_dirs,
                    includes,
                    external,
                    visited,
                );
            }
            None if inc.angle => {
                external.insert(inc.path.clone());
            }
            None => {
                warn!("{path}: included file not found: {inc}", inc = inc.path);
            }
        }
    }
}

/// Resolve an `#include` the way a compiler would: quoted includes are
/// first looked up relative to the including file's directory, then (like
/// angle-bracket includes) in each include directory in order.
fn resolve(
    index: &Index,
    include_dirs: &[String],
    includer: &str,
    inc: &Include,
) -> Option<String> {
    if !inc.angle {
        let candidate = normalize(&join(dirname(includer), &inc.path));
        if index.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    for dir in include_dirs {
        let candidate = normalize(&join(dir, &inc.path));
        if index.contains_key(&candidate) {
            return Some(candidate);
        }
    }
    None
}

fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(pos) => &path[..pos],
        None => "",
    }
}

fn join(dir: &str, name: &str) -> String {
    if dir.is_empty() {
        name.to_string()
    } else {
        format!("{dir}/{name}")
    }
}

/// Collapse `.` and `..` components and redundant slashes.
fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = vec![];
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            _ => parts.push(part),
        }
    }
    parts.join("/")
}

fn extract_includes(contents: &[u8]) -> Vec<Include> {
    let mut includes = vec![];
    let Ok(text) = std::str::from_utf8(contents) else {
        return includes;
    };
    // FIXME: process #ifdefs so we can skip #includes that don't apply.
    for line in text.lines() {
        let Some(rest) = line.trim().strip_prefix('#') else {
            continue;
        };
        let Some(rest) = rest.trim_start().strip_prefix("include") else {
            continue;
        };
        let rest = rest.trim_start();
        let (close, angle) = match rest.chars().next() {
            Some('"') => ('"', false),
            Some('<') => ('>', true),
            _ => continue,
        };
        let rest = &rest[1..];
        if let Some(end) = rest.find(close) {
            includes.push(Include {
                path: rest[..end].to_string(),
                angle,
            });
        }
    }
    includes
}

//! `#include` dependency scanner for using Nix as a C/C++ build system.
//!
//! See README.md for the argument and result schema of `getDeps`.

mod preprocessor;

use nix_wasm_rust::{warn, Value};
use preprocessor::{eval_condition, ConditionalStack, Defines};
use std::cell::OnceCell;
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

/// An `#include` directive: the included path and whether it used angle brackets.
struct Include {
    path: String,
    angle: bool,
}

enum Source {
    /// A real file, denoted by a Nix path or string value.
    Real(Value),
    /// A file generated at build time (e.g. by bison) from the real file
    /// `from`, whose `#include`s it is assumed to inherit.
    Generated { from: String },
}

struct FileInfo {
    source: Source,
    /// The `#include` directives of this file, parsed lazily the first
    /// time the file is reached from a compilation unit, so that only
    /// files that are actually needed are read. Unused for generated
    /// files, which delegate to their source.
    includes: OnceCell<Vec<Include>>,
}

impl FileInfo {
    fn new(value: Value) -> Self {
        FileInfo {
            source: Source::Real(value),
            includes: OnceCell::new(),
        }
    }

    fn generated(from: String) -> Self {
        FileInfo {
            source: Source::Generated { from },
            includes: OnceCell::new(),
        }
    }
}

/// Index of all known files, keyed by their path relative to the root namespace.
type Index = BTreeMap<String, FileInfo>;

/// The `#include`s of an indexed file.
fn includes_of<'a>(index: &'a Index, path: &str, defines: &Defines) -> &'a [Include] {
    let file = &index[path];
    match &file.source {
        Source::Real(value) => file
            .includes
            .get_or_init(|| extract_includes(&value.read_file(), defines)),
        Source::Generated { from } => includes_of(index, from, defines),
    }
}

#[no_mangle]
pub extern "C" fn getDeps(args: Value) -> Value {
    let builtins = args
        .get_attr("builtins")
        .expect("missing 'builtins' argument");
    let read_dir = builtins.get_attr("readDir").unwrap();

    let source_extensions =
        get_string_list(&args, "sourceExtensions").expect("missing 'sourceExtensions' argument");

    // Macros known to be defined or undefined, for evaluating conditionals.
    let defines = Defines {
        defined: args
            .get_attr("defines")
            .map(|v| {
                v.get_attrset()
                    .into_iter()
                    .map(|(k, v)| (k, v.get_string()))
                    .collect::<HashMap<_, _>>()
            })
            .unwrap_or_default(),
        undefined: get_string_list(&args, "undefines")
            .unwrap_or_default()
            .into_iter()
            .collect(),
    };

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
            index.insert(normalize(&name), FileInfo::new(value));
        }
    }

    // Files generated at build time, which inherit the includes of their source.
    if let Some(generated) = args.get_attr("generated") {
        for (name, from) in generated.get_attrset() {
            let from = normalize(&from.get_string());
            if !matches!(
                index.get(&from),
                Some(FileInfo {
                    source: Source::Real(_),
                    ..
                })
            ) {
                panic!("source '{from}' of generated file '{name}' is not a known file");
            }
            index.insert(normalize(&name), FileInfo::generated(from));
        }
    }

    let include_dirs: Vec<String> = get_string_list(&args, "includeDirs")
        .unwrap_or_default()
        .iter()
        .map(|d| normalize(d))
        .collect();

    // The compilation units: either given explicitly, or every indexed
    // file with a source extension. In both cases minus `excludeSources`.
    let exclude_sources: Vec<String> = get_string_list(&args, "excludeSources")
        .unwrap_or_default()
        .iter()
        .map(|d| normalize(d))
        .collect();
    let is_excluded = |path: &str| {
        exclude_sources
            .iter()
            .any(|e| path == e || path.starts_with(&format!("{e}/")))
    };

    let candidates: Vec<String> = match get_string_list(&args, "sources") {
        Some(sources) => sources.iter().map(|s| normalize(s)).collect(),
        None => index
            .keys()
            .filter(|path| has_extension(path, &source_extensions))
            .cloned()
            .collect(),
    };
    let sources: BTreeSet<String> = candidates
        .into_iter()
        .filter(|path| !is_excluded(path))
        .collect();

    let mut warned: HashSet<(String, String)> = HashSet::new();
    let mut results = vec![];
    for source in &sources {
        let Some(file) = index.get(source) else {
            panic!("source file '{source}' not found in the scanned roots or explicit files");
        };

        let mut includes: BTreeMap<String, Value> = BTreeMap::new();
        let mut generated: BTreeSet<String> = BTreeSet::new();
        let mut external: BTreeSet<String> = BTreeSet::new();
        let mut visited: HashSet<String> = HashSet::new();
        collect_transitive_includes(
            source,
            &index,
            &include_dirs,
            &defines,
            &mut includes,
            &mut generated,
            &mut external,
            &mut visited,
            &mut warned,
        );

        let include_attrs: Vec<(&str, Value)> = includes
            .iter()
            .map(|(path, value)| (path.as_str(), *value))
            .collect();
        let generated_values: Vec<Value> =
            generated.iter().map(|s| Value::make_string(s)).collect();
        let external_values: Vec<Value> = external.iter().map(|s| Value::make_string(s)).collect();
        let src = match &file.source {
            Source::Real(value) => *value,
            Source::Generated { .. } => Value::make_null(),
        };

        results.push(Value::make_attrset(&[
            ("path", Value::make_string(source)),
            ("src", src),
            ("includes", Value::make_attrset(&include_attrs)),
            ("generatedIncludes", Value::make_list(&generated_values)),
            ("externalIncludes", Value::make_list(&external_values)),
        ]));
    }

    Value::make_list(&results)
}

/// Get an optional list-of-strings attribute.
fn get_string_list(args: &Value, name: &str) -> Option<Vec<String>> {
    args.get_attr(name)
        .map(|v| v.get_list().iter().map(|s| s.get_string()).collect())
}

fn has_extension(name: &str, extensions: &[String]) -> bool {
    extensions.iter().any(|ext| name.ends_with(ext.as_str()))
}

/// Add every regular file under `dir` to the index. Files are not read here;
/// see `FileInfo::includes`.
fn scan_files(read_dir: &Value, dir: &Value, prefix: &str, index: &mut Index) {
    for (name, file_type) in read_dir.call(&[*dir]).get_attrset() {
        let child = dir.make_path(&name);
        let path = join(prefix, &name);
        match file_type.get_string().as_str() {
            "regular" => {
                index.insert(path, FileInfo::new(child));
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
    defines: &Defines,
    includes: &mut BTreeMap<String, Value>,
    generated: &mut BTreeSet<String>,
    external: &mut BTreeSet<String>,
    visited: &mut HashSet<String>,
    warned: &mut HashSet<(String, String)>,
) {
    if !visited.insert(path.to_string()) {
        return;
    }

    for inc in includes_of(index, path, defines) {
        match resolve(index, include_dirs, path, inc) {
            Some(resolved) => {
                match &index[&resolved].source {
                    Source::Real(value) => {
                        includes.insert(resolved.clone(), *value);
                    }
                    Source::Generated { .. } => {
                        generated.insert(resolved.clone());
                    }
                }
                collect_transitive_includes(
                    &resolved,
                    index,
                    include_dirs,
                    defines,
                    includes,
                    generated,
                    external,
                    visited,
                    warned,
                );
            }
            None if inc.angle => {
                external.insert(inc.path.clone());
            }
            None => {
                // Warn once per (file, include), not once per unit reaching it.
                if warned.insert((path.to_string(), inc.path.clone())) {
                    warn!("{path}: included file not found: {inc}", inc = inc.path);
                }
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

fn extract_includes(contents: &[u8], defines: &Defines) -> Vec<Include> {
    let mut includes = vec![];
    let Ok(text) = std::str::from_utf8(contents) else {
        return includes;
    };

    let mut conditionals = ConditionalStack::new();

    for line in logical_lines(text) {
        let Some(directive) = line.trim().strip_prefix('#') else {
            continue;
        };
        let directive = strip_comments(directive);
        let directive = directive.trim();
        let (name, rest) = match directive.find(|c: char| !c.is_ascii_alphanumeric() && c != '_') {
            Some(pos) => (&directive[..pos], directive[pos..].trim()),
            None => (directive, ""),
        };

        match name {
            "if" => conditionals.push(eval_condition(rest, defines)),
            "ifdef" => conditionals.push(eval_condition(&format!("defined({rest})"), defines)),
            "ifndef" => conditionals.push(eval_condition(&format!("!defined({rest})"), defines)),
            "elif" => conditionals.elif(eval_condition(rest, defines)),
            "elifdef" => conditionals.elif(eval_condition(&format!("defined({rest})"), defines)),
            "elifndef" => conditionals.elif(eval_condition(&format!("!defined({rest})"), defines)),
            "else" => conditionals.else_(),
            "endif" => conditionals.pop(),
            "include" if conditionals.active() => {
                let (close, angle) = match rest.chars().next() {
                    Some('"') => ('"', false),
                    Some('<') => ('>', true),
                    _ => continue,
                };
                if let Some(end) = rest[1..].find(close) {
                    includes.push(Include {
                        path: rest[1..1 + end].to_string(),
                        angle,
                    });
                }
            }
            _ => {}
        }
    }
    includes
}

/// Split into lines, joining lines that end with a backslash.
fn logical_lines(text: &str) -> Vec<String> {
    let mut lines = vec![];
    let mut current = String::new();
    for line in text.lines() {
        if let Some(prefix) = line.strip_suffix('\\') {
            current.push_str(prefix);
            current.push(' ');
        } else {
            current.push_str(line);
            lines.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        lines.push(current);
    }
    lines
}

/// Remove `// ...` and `/* ... */` comments from a directive line.
fn strip_comments(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    loop {
        let line_comment = rest.find("//");
        let block_comment = rest.find("/*");
        match (line_comment, block_comment) {
            (Some(l), Some(b)) if l < b => {
                out.push_str(&rest[..l]);
                break;
            }
            (Some(l), None) => {
                out.push_str(&rest[..l]);
                break;
            }
            (_, Some(b)) => {
                out.push_str(&rest[..b]);
                out.push(' ');
                match rest[b + 2..].find("*/") {
                    Some(e) => rest = &rest[b + 2 + e + 2..],
                    None => break,
                }
            }
            (None, None) => {
                out.push_str(rest);
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defines() -> Defines {
        Defines {
            defined: [("__linux__", "1"), ("HAVE_FOO", "0")]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .collect(),
            undefined: ["_WIN32"].into_iter().map(str::to_string).collect(),
        }
    }

    fn includes(source: &str) -> Vec<String> {
        extract_includes(source.as_bytes(), &defines())
            .into_iter()
            .map(|inc| {
                if inc.angle {
                    format!("<{}>", inc.path)
                } else {
                    format!("\"{}\"", inc.path)
                }
            })
            .collect()
    }

    #[test]
    fn plain_includes() {
        assert_eq!(
            includes("#include \"a.hh\"\n  #  include <b.h> // comment\n#include \"c.hh\" /* x */\nint x;\n"),
            ["\"a.hh\"", "<b.h>", "\"c.hh\""]
        );
    }

    #[test]
    fn conditionals() {
        let source = "\
#ifdef _WIN32
#  include \"win.hh\"
#elif defined(__linux__)
#  include \"linux.hh\"
#else
#  include \"other.hh\"
#endif
#if HAVE_FOO
#  include \"foo.hh\"
#endif
#if 0
#  include \"dead.hh\"
#endif
#ifndef UNKNOWN
#  include \"maybe1.hh\"
#else
#  include \"maybe2.hh\"
#endif
#if defined(__linux__) && UNKNOWN_VERSION >= 3
#  include \"maybe3.hh\"
#endif
#include \"always.hh\"
";
        assert_eq!(
            includes(source),
            [
                "\"linux.hh\"",
                "\"maybe1.hh\"",
                "\"maybe2.hh\"",
                "\"maybe3.hh\"",
                "\"always.hh\""
            ]
        );
    }

    #[test]
    fn nested_conditionals() {
        let source = "\
#ifndef _WIN32
#  ifdef __linux__
#    include \"linux.hh\"
#  else
#    include \"unix.hh\"
#  endif
#else
#  ifdef __linux__
#    include \"impossible.hh\"
#  endif
#endif
";
        assert_eq!(includes(source), ["\"linux.hh\""]);
    }

    #[test]
    fn line_continuations_and_comments() {
        let source = "\
#if defined(_WIN32) || \\
    defined(__APPLE__)
#  include \"not-linux.hh\"
#endif /* end */
#if /* inline */ defined(__linux__) // trailing
#  include \"linux.hh\"
#endif
";
        // `__APPLE__` is unknown here, so the first block is kept.
        assert_eq!(includes(source), ["\"not-linux.hh\"", "\"linux.hh\""]);
    }

    #[test]
    fn non_utf8_has_no_includes() {
        assert!(extract_includes(&[0xff, 0xfe, b'#'], &defines()).is_empty());
    }

    #[test]
    fn normalize_paths() {
        assert_eq!(normalize("a/b/../c/./d.hh"), "a/c/d.hh");
        assert_eq!(normalize("../x.hh"), "x.hh");
        assert_eq!(normalize("unix/"), "unix");
        assert_eq!(normalize(""), "");
    }
}

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

/// Where an `#include` directive points to.
enum Resolution {
    /// A file in the index.
    File(String),
    /// An angle-bracket include not found in the index.
    External(String),
}

/// What is extracted from a file's contents.
struct Parsed {
    includes: Vec<Include>,
    /// The known macros (see `Defines`) that occur in the file, so that
    /// a compilation unit can be told which macros it is sensitive to.
    used_defines: Vec<String>,
}

struct FileInfo {
    source: Source,
    /// The contents of this file, parsed lazily the first time the file
    /// is reached from a compilation unit, so that only files that are
    /// actually needed are read. Unused for generated files, which
    /// delegate to their source.
    parsed: OnceCell<Parsed>,
    /// The resolved `#include`s, computed once per file rather than once
    /// per unit reaching it.
    resolved: OnceCell<Vec<Resolution>>,
}

impl FileInfo {
    fn new(value: Value) -> Self {
        FileInfo {
            source: Source::Real(value),
            parsed: OnceCell::new(),
            resolved: OnceCell::new(),
        }
    }

    fn generated(from: String) -> Self {
        FileInfo {
            source: Source::Generated { from },
            parsed: OnceCell::new(),
            resolved: OnceCell::new(),
        }
    }
}

/// Index of all known files, keyed by their path relative to the root namespace.
type Index = BTreeMap<String, FileInfo>;

/// What a unit's macro sensitivity is computed against: the macros to
/// track (see `trackedDefines`), with their Nix values passed through.
type Tracked = BTreeMap<String, Value>;

/// The parsed contents of an indexed file.
fn parsed_of<'a>(index: &'a Index, path: &str, defines: &Defines, tracked: &Tracked) -> &'a Parsed {
    let file = &index[path];
    match &file.source {
        Source::Real(value) => file
            .parsed
            .get_or_init(|| parse_file(&value.read_file(), defines, tracked)),
        Source::Generated { from } => parsed_of(index, from, defines, tracked),
    }
}

fn parse_file(contents: &[u8], defines: &Defines, tracked: &Tracked) -> Parsed {
    Parsed {
        includes: extract_includes(contents, defines),
        used_defines: find_used_defines(contents, tracked),
    }
}

/// The tracked macros that occur in `contents`, found by scanning for
/// identifiers. Comments and strings are not skipped: this may report a
/// macro that is only mentioned, but never misses one that is used.
fn find_used_defines(contents: &[u8], tracked: &Tracked) -> Vec<String> {
    let mut used = BTreeSet::new();
    let mut i = 0;
    while i < contents.len() {
        let c = contents[i];
        if c.is_ascii_alphabetic() || c == b'_' {
            let start = i;
            while i < contents.len() && (contents[i].is_ascii_alphanumeric() || contents[i] == b'_')
            {
                i += 1;
            }
            if let Ok(ident) = std::str::from_utf8(&contents[start..i]) {
                if tracked.contains_key(ident) {
                    used.insert(ident.to_string());
                }
            }
        } else if c.is_ascii_digit() {
            // Skip numbers (and their suffixes) so that e.g. `0xFF` is not an identifier.
            while i < contents.len() && (contents[i].is_ascii_alphanumeric() || contents[i] == b'_')
            {
                i += 1;
            }
        } else {
            i += 1;
        }
    }
    used.into_iter().collect()
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

    // Macros whose use is reported per unit, with their values (which may
    // be anything, including null for "undefined"; they are passed through).
    let tracked: Tracked = args
        .get_attr("trackedDefines")
        .map(|v| v.get_attrset())
        .unwrap_or_default();

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

    let mut results = vec![];
    for source in &sources {
        let Some(file) = index.get(source) else {
            panic!("source file '{source}' not found in the scanned roots or explicit files");
        };

        let mut includes: BTreeMap<&str, Value> = BTreeMap::new();
        let mut generated: BTreeSet<&str> = BTreeSet::new();
        let mut external: BTreeSet<&str> = BTreeSet::new();
        let mut used: BTreeSet<&str> = BTreeSet::new();
        let mut visited: HashSet<&str> = HashSet::new();
        visited.insert(source);
        collect_transitive_includes(
            source,
            &index,
            &include_dirs,
            &defines,
            &tracked,
            &mut includes,
            &mut generated,
            &mut external,
            &mut used,
            &mut visited,
        );

        let include_attrs: Vec<(&str, Value)> = includes
            .iter()
            .map(|(path, value)| (*path, *value))
            .collect();
        let generated_values: Vec<Value> =
            generated.iter().map(|s| Value::make_string(s)).collect();
        let external_values: Vec<Value> = external.iter().map(|s| Value::make_string(s)).collect();
        let used_attrs: Vec<(&str, Value)> = used.iter().map(|s| (*s, tracked[*s])).collect();
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
            ("usedDefines", Value::make_attrset(&used_attrs)),
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

/// The resolved `#include`s of an indexed file, memoized per file.
fn resolved_includes_of<'a>(
    index: &'a Index,
    path: &str,
    include_dirs: &[String],
    defines: &Defines,
    tracked: &Tracked,
) -> &'a [Resolution] {
    let file = &index[path];
    if let Source::Generated { from } = &file.source {
        // Generated files live next to their source, so resolve as it.
        return resolved_includes_of(index, from, include_dirs, defines, tracked);
    }
    file.resolved.get_or_init(|| {
        parsed_of(index, path, defines, tracked)
            .includes
            .iter()
            .filter_map(|inc| match resolve(index, include_dirs, path, inc) {
                Some(resolved) => Some(Resolution::File(resolved)),
                None if inc.angle => Some(Resolution::External(inc.path.clone())),
                None => {
                    warn!("{path}: included file not found: {inc}", inc = inc.path);
                    None
                }
            })
            .collect()
    })
}

/// Walk the include graph from `path`, collecting every file reached.
fn collect_transitive_includes<'a>(
    path: &str,
    index: &'a Index,
    include_dirs: &[String],
    defines: &Defines,
    tracked: &Tracked,
    includes: &mut BTreeMap<&'a str, Value>,
    generated: &mut BTreeSet<&'a str>,
    external: &mut BTreeSet<&'a str>,
    used: &mut BTreeSet<&'a str>,
    visited: &mut HashSet<&'a str>,
) {
    used.extend(
        parsed_of(index, path, defines, tracked)
            .used_defines
            .iter()
            .map(String::as_str),
    );

    for resolution in resolved_includes_of(index, path, include_dirs, defines, tracked) {
        match resolution {
            Resolution::File(resolved) => {
                let (resolved, file) = index.get_key_value(resolved.as_str()).unwrap();
                if !visited.insert(resolved) {
                    continue;
                }
                match &file.source {
                    Source::Real(value) => {
                        includes.insert(resolved, *value);
                    }
                    Source::Generated { .. } => {
                        generated.insert(resolved);
                    }
                }
                collect_transitive_includes(
                    resolved,
                    index,
                    include_dirs,
                    defines,
                    tracked,
                    includes,
                    generated,
                    external,
                    used,
                    visited,
                );
            }
            Resolution::External(inc) => {
                external.insert(inc);
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

#[cfg(test)]
mod used_defines_tests {
    use super::*;

    #[test]
    fn finds_used_defines() {
        // The values are opaque here; only the names matter.
        let tracked: Tracked = ["HAVE_FOO", "VERSION", "IS_STATIC"]
            .into_iter()
            .map(|k| (k.to_string(), Value::from_raw(1)))
            .collect();
        let source = "\
#if HAVE_FOO
int x = HAVE_FOOBAR; // HAVE_FOOBAR is a different macro
#endif
#ifndef IS_STATIC
const char * v = VERSION_STRING; // a different identifier
#endif
int hex = 0xVERSION; /* a number, not an identifier */
";
        assert_eq!(
            find_used_defines(source.as_bytes(), &tracked),
            ["HAVE_FOO", "IS_STATIC"]
        );
        // A mention in a comment counts: this is an over-approximation.
        assert_eq!(find_used_defines(b"// see VERSION", &tracked), ["VERSION"]);
        assert!(find_used_defines(b"FOO_HAVE_FOO", &tracked).is_empty());
    }
}

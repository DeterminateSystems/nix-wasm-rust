use nix_wasm_rust::{Type, Value};
use serde_json::{Map, Number};

// `jsonschema`'s dependency `ahash` calls `getrandom::fill()` at runtime to
// seed its `RandomState`. Since Nix's `builtins.wasm` runs modules in
// a bare Wasmtime instance with no JS engine, the default entropy source is
// not available. This custom backend supplies entropy directly. It only
// needs to vary ahash's hash output--not be cryptographically secure.
// 
// See: https://docs.rs/getrandom/0.3.2/getrandom/#custom-backend
#[unsafe(no_mangle)]
unsafe extern "Rust" fn __getrandom_v03_custom(
    dest: *mut u8,
    len: usize,
) -> Result<(), getrandom::Error> {
    let mut state: u64 = 0x9E3779B97F4A7C15 ^ (dest as u64);
    for i in 0..len {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        unsafe {
            *dest.add(i) = (state & 0xff) as u8;
        }
    }
    Ok(())
}

fn nix_to_json(v: Value) -> serde_json::Value {
    match v.get_type() {
        Type::Int => serde_json::Value::Number(Number::from(v.get_int())),
        Type::Float => Number::from_f64(v.get_float())
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Type::Bool => serde_json::Value::Bool(v.get_bool()),
        Type::String => serde_json::Value::String(v.get_string()),
        Type::Null => serde_json::Value::Null,
        Type::List => serde_json::Value::Array(v.get_list().into_iter().map(nix_to_json).collect()),
        Type::Attrs => {
            let mut map = Map::new();
            for (key, value) in v.get_attrset() {
                map.insert(key, nix_to_json(value));
            }
            serde_json::Value::Object(map)
        }
        other => panic!("Nix type {} cannot be converted to JSON", other as u64),
    }
}

#[no_mangle]
pub extern "C" fn validateJSONSchema(arg: Value) -> Value {
    let schema_arg = arg.get_attr("schema").expect("missing 'schema' argument");
    let instance_arg = arg
        .get_attr("instance")
        .expect("missing 'instance' argument");

    let schema_json = schema_arg.get_string();
    let schema: serde_json::Value = serde_json::from_str(&schema_json)
        .unwrap_or_else(|err| panic!("invalid JSON schema JSON: {err}"));
    let instance = nix_to_json(instance_arg);

    let validator = jsonschema::validator_for(&schema)
        .unwrap_or_else(|err| panic!("invalid JSON schema: {err}"));

    let errors: Vec<Value> = validator
        .iter_errors(&instance)
        .map(|error| {
            Value::make_attrset(&[
                ("message", Value::make_string(&error.to_string())),
                (
                    "instancePath",
                    Value::make_string(&error.instance_path().to_string()),
                ),
                (
                    "schemaPath",
                    Value::make_string(&error.schema_path().to_string()),
                ),
            ])
        })
        .collect();

    Value::make_attrset(&[
        ("valid", Value::make_bool(errors.is_empty())),
        ("errors", Value::make_list(&errors)),
    ])
}

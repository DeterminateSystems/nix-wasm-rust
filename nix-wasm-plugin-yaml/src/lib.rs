use nix_wasm_rust::{Type, Value};
use yaml_rust2::{Yaml, YamlEmitter, YamlLoader};

fn yaml_to_value(yaml: &Yaml) -> Value {
    match yaml {
        Yaml::Real(_) => Value::make_float(yaml.as_f64().expect("YAML floating point number")),
        Yaml::Integer(n) => Value::make_int(*n),
        Yaml::String(s) => Value::make_string(s),
        Yaml::Boolean(b) => Value::make_bool(*b),
        Yaml::Array(array) => {
            Value::make_list(&array.iter().map(yaml_to_value).collect::<Vec<_>>())
        }
        Yaml::Hash(hash) => Value::make_attrset(
            &hash
                .iter()
                .map(|(key, value)| {
                    let key: &str = match &key {
                        Yaml::String(s) => s,
                        _ => panic!("non-string YAML keys are not supported, in: {:?}", key),
                    };
                    (key, yaml_to_value(value))
                })
                .collect::<Vec<_>>(),
        ),
        Yaml::Null => Value::make_null(),
        _ => panic!("unimplemented YAML value: {:?}", yaml),
    }
}

#[no_mangle]
pub extern "C" fn fromYAML(arg: Value) -> Value {
    Value::make_list(
        &YamlLoader::load_from_str(&arg.get_string())
            .unwrap()
            .iter()
            .map(yaml_to_value)
            .collect::<Vec<_>>(),
    )
}

fn to_yaml(v: Value) -> Yaml {
    match v.get_type() {
        Type::Int => Yaml::Integer(v.get_int()),
        Type::Float => Yaml::Real(format!("{}", v.get_float())),
        Type::Bool => Yaml::Boolean(v.get_bool()),
        Type::String => Yaml::String(v.get_string()),
        Type::Null => Yaml::Null,
        Type::Attrs => Yaml::Hash(
            v.get_attrset()
                .into_iter()
                .map(|(key, value)| (Yaml::String(key), to_yaml(value)))
                .collect(),
        ),
        Type::List => Yaml::Array(v.get_list().into_iter().map(to_yaml).collect::<Vec<_>>()),
        _ => panic!(
            "Nix type {} cannot be serialized to YAML",
            v.get_type() as u64
        ),
    }
}

#[no_mangle]
pub extern "C" fn toYAML(arg: Value) -> Value {
    let mut out_str = String::new();

    for v in arg.get_list() {
        let yaml = to_yaml(v);
        let mut emitter = YamlEmitter::new(&mut out_str);
        emitter.dump(&yaml).unwrap();
        out_str.push('\n');
    }

    Value::make_string(&out_str)
}

use nix_wasm_rust::{Type, Value};
use yaml_rust2::{yaml, Yaml, YamlEmitter, YamlLoader};

fn yaml_to_value(yaml: &Yaml) -> Value {
    match yaml {
        Yaml::Real(_) => Value::make_float(yaml.as_f64().expect("YAML floating point number")),
        Yaml::Integer(n) => Value::make_int(*n),
        Yaml::String(s) => Value::make_string(s),
        Yaml::Boolean(b) => Value::make_bool(*b),
        Yaml::Array(array) => {
            let mut res = vec![];
            for value in array {
                res.push(yaml_to_value(value));
            }
            Value::make_list(&res)
        }
        Yaml::Hash(hash) => {
            let mut attrset = vec![];
            for (key, value) in hash {
                let key: &str = match &key {
                    Yaml::String(s) => s,
                    _ => panic!("non-string YAML keys are not supported, in: {:?}", key),
                };
                attrset.push((key, yaml_to_value(value)));
            }
            Value::make_attrset(&attrset)
        }
        Yaml::Null => Value::make_null(),
        _ => panic!("unimplemented YAML value: {:?}", yaml),
    }
}

#[no_mangle]
pub extern "C" fn fromYAML(arg: Value) -> Value {
    let s = arg.get_string();

    let yaml = YamlLoader::load_from_str(&s).unwrap();

    let mut docs = vec![];

    for doc in yaml {
        docs.push(yaml_to_value(&doc));
    }

    Value::make_list(&docs)
}

fn to_yaml(v: Value) -> Yaml {
    match v.get_type() {
        Type::Int => Yaml::Integer(v.get_int()),
        Type::Float => Yaml::Real(format!("{}", v.get_float())),
        Type::Bool => Yaml::Boolean(v.get_bool()),
        Type::String => Yaml::String(v.get_string()),
        Type::Null => Yaml::Null,
        Type::Attrs => {
            let mut hash = yaml::Hash::new();
            for (key, value) in v.get_attrset() {
                hash.insert(Yaml::String(key), to_yaml(value));
            }
            Yaml::Hash(hash)
        }
        Type::List => {
            let mut array = yaml::Array::new();
            for value in v.get_list() {
                array.push(to_yaml(value));
            }
            Yaml::Array(array)
        }
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

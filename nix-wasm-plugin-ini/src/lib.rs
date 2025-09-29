use ini::Ini;
use nix_wasm_rust::Value;

#[no_mangle]
pub extern "C" fn fromINI(arg: Value) -> Value {
    let s = arg.get_string();

    let ini = Ini::load_from_str(&s).expect("Could not parse INI file");

    let mut sections = vec![];

    for (section_name, section) in ini.iter() {
        let mut props_attrset = vec![];
        for (prop_name, prop_value) in section {
            props_attrset.push((prop_name, Value::make_string(prop_value)));
        }
        sections.push((
            section_name.unwrap_or(""),
            Value::make_attrset(&props_attrset),
        ));
    }

    Value::make_attrset(&sections)
}

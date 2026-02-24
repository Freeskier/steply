use crate::core::value::Value;

pub fn normalize_ascii_key(input: &str, fallback: &str) -> String {
    let mut key = String::new();
    let mut previous_underscore = false;
    for ch in input.chars() {
        if ch.is_ascii_alphanumeric() {
            key.push(ch.to_ascii_lowercase());
            previous_underscore = false;
            continue;
        }
        if !previous_underscore && !key.is_empty() {
            key.push('_');
            previous_underscore = true;
        }
    }

    while key.ends_with('_') {
        key.pop();
    }

    if key.is_empty() {
        fallback.to_string()
    } else {
        key
    }
}

pub fn seed_value_from_record(
    seed: Option<&Value>,
    index: usize,
    key: &str,
    label: &str,
) -> Option<Value> {
    let seed = seed?;
    match seed {
        Value::Object(map) => map.get(key).cloned().or_else(|| map.get(label).cloned()),
        Value::List(items) => items.get(index).cloned(),
        Value::None => None,
        scalar if index == 0 => Some(scalar.clone()),
        _ => None,
    }
}

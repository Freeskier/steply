mod validate;

fn invalid_yaml_message(raw: &str) -> String {
    match super::load_from_yaml_str(raw) {
        Ok(_) => panic!("expected invalid yaml config"),
        Err(err) => err.to_string(),
    }
}

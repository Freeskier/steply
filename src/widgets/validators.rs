pub type ValidationError = String;
pub type Validator = Box<dyn Fn(&str) -> Result<(), ValidationError> + Send + Sync>;

/// Run a list of validators against `value`, returning the first error.
pub fn run_validators(validators: &[Validator], value: &str) -> Result<(), String> {
    for validator in validators {
        validator(value)?;
    }
    Ok(())
}

pub fn required(message: impl Into<String>) -> Validator {
    let message = message.into();
    Box::new(move |value: &str| {
        if value.trim().is_empty() {
            Err(message.clone())
        } else {
            Ok(())
        }
    })
}

pub fn min_length(min_len: usize, message: impl Into<String>) -> Validator {
    let message = message.into();
    Box::new(move |value: &str| {
        if value.chars().count() < min_len {
            Err(message.clone())
        } else {
            Ok(())
        }
    })
}

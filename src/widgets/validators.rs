use crate::core::value::Value;

pub type Validator = Box<dyn Fn(&Value) -> Result<(), String> + Send + Sync>;


pub fn run_validators(validators: &[Validator], value: &Value) -> Result<(), String> {
    for v in validators {
        v(value)?;
    }
    Ok(())
}


pub fn required() -> Validator {
    Box::new(|v| {
        if v.is_empty() {
            Err("This field is required.".into())
        } else {
            Ok(())
        }
    })
}


pub fn required_msg(msg: impl Into<String> + 'static) -> Validator {
    let msg = msg.into();
    Box::new(move |v| {
        if v.is_empty() {
            Err(msg.clone())
        } else {
            Ok(())
        }
    })
}



pub fn min_length(n: usize) -> Validator {
    Box::new(move |v| {
        if let Value::Text(s) = v {
            if s.chars().count() < n {
                return Err(format!("Minimum {n} characters required."));
            }
        }
        Ok(())
    })
}


pub fn max_length(n: usize) -> Validator {
    Box::new(move |v| {
        if let Value::Text(s) = v {
            if s.chars().count() > n {
                return Err(format!("Maximum {n} characters allowed."));
            }
        }
        Ok(())
    })
}


pub fn min_selections(n: usize) -> Validator {
    Box::new(move |v| {
        if let Value::List(items) = v {
            if items.len() < n {
                return Err(format!("Select at least {n} option(s)."));
            }
        }
        Ok(())
    })
}


pub fn max_selections(n: usize) -> Validator {
    Box::new(move |v| {
        if let Value::List(items) = v {
            if items.len() > n {
                return Err(format!("Select at most {n} option(s)."));
            }
        }
        Ok(())
    })
}


pub fn must_be_checked() -> Validator {
    Box::new(|v| match v {
        Value::Bool(true) => Ok(()),
        _ => Err("This field must be checked.".into()),
    })
}


pub fn min_value(n: f64) -> Validator {
    Box::new(move |v| {
        if let Some(num) = v.as_number() {
            if num < n {
                return Err(format!("Value must be at least {n}."));
            }
        }
        Ok(())
    })
}


pub fn max_value(n: f64) -> Validator {
    Box::new(move |v| {
        if let Some(num) = v.as_number() {
            if num > n {
                return Err(format!("Value must be at most {n}."));
            }
        }
        Ok(())
    })
}

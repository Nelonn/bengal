use sparkler::Value;

pub fn native_json_stringify(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "stringify requires at least one argument".to_string(),
        ));
    }

    match simd_json::to_string(&args[0]) {
        Ok(s) => Ok(Value::String(s)),
        Err(e) => Err(Value::String(format!("Failed to serialize: {}", e))),
    }
}

pub fn native_json_parse(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String(
            "parse requires at least one argument".to_string(),
        ));
    }

    let json_str = match &args[0] {
        Value::String(s) => s.clone(),
        _ => {
            return Err(Value::String(
                "parse requires a string argument".to_string(),
            ))
        }
    };

    let mut bytes = json_str.into_bytes();
    match simd_json::from_slice(&mut bytes) {
        Ok(v) => Ok(v),
        Err(e) => Err(Value::String(format!("Failed to parse JSON: {}", e))),
    }
}

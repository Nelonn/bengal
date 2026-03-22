use sparkler::{Value, NativeResult};

pub fn native_json_stringify(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::String(
            "stringify requires at least one argument".to_string(),
        ));
    }

    match simd_json::to_string(&args[0]) {
        Ok(s) => NativeResult::Ready(Value::String(s)),
        Err(e) => NativeResult::Ready(Value::String(format!("Failed to serialize: {}", e))),
    }
}

pub fn native_json_parse(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::String(
            "parse requires at least one argument".to_string(),
        ));
    }

    let json_str = match &args[0] {
        Value::String(s) => s.clone(),
        _ => {
            return NativeResult::Ready(Value::String(
                "parse requires a string argument".to_string(),
            ))
        }
    };

    let mut bytes = json_str.into_bytes();
    match simd_json::from_slice(&mut bytes) {
        Ok(v) => NativeResult::Ready(v),
        Err(e) => NativeResult::Ready(Value::String(format!("Failed to parse JSON: {}", e))),
    }
}

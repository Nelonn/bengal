use sparkler::Value;

/// Native str() function that converts any value to string
pub fn native_str(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::String("".to_string()));
    }
    
    // Convert the first argument to string
    let result = match &args[0] {
        Value::String(s) => Value::String(s.clone()),
        Value::Int8(n) => Value::String(n.to_string()),
        Value::Int16(n) => Value::String(n.to_string()),
        Value::Int32(n) => Value::String(n.to_string()),
        Value::Int64(n) => Value::String(n.to_string()),
        Value::UInt8(n) => Value::String(n.to_string()),
        Value::UInt16(n) => Value::String(n.to_string()),
        Value::UInt32(n) => Value::String(n.to_string()),
        Value::UInt64(n) => Value::String(n.to_string()),
        Value::Float32(n) => Value::String(n.to_string()),
        Value::Float64(n) => Value::String(n.to_string()),
        Value::Bool(b) => Value::String(b.to_string()),
        Value::Null => Value::String("null".to_string()),
        Value::Instance(inst) => {
            let inst = inst.lock().unwrap();
            let mut fields_str = Vec::new();
            for (key, value) in &inst.fields {
                let value_str = match value {
                    Value::String(s) => format!("\"{}\"", s),
                    Value::Int8(n) => n.to_string(),
                    Value::Int16(n) => n.to_string(),
                    Value::Int32(n) => n.to_string(),
                    Value::Int64(n) => n.to_string(),
                    Value::UInt8(n) => n.to_string(),
                    Value::UInt16(n) => n.to_string(),
                    Value::UInt32(n) => n.to_string(),
                    Value::UInt64(n) => n.to_string(),
                    Value::Float32(n) => n.to_string(),
                    Value::Float64(n) => n.to_string(),
                    Value::Bool(b) => b.to_string(),
                    Value::Null => "null".to_string(),
                    Value::Instance(_) => "[instance]".to_string(),
                    Value::Array(_) => "[array]".to_string(),
                    Value::Promise(_) => "[promise]".to_string(),
                    Value::Exception(e) => format!("[exception: {}]", e.message),
                };
                fields_str.push(format!("\"{}\": {}", key, value_str));
            }
            Value::String(format!("{{ {} }}", fields_str.join(", ")))
        }
        Value::Array(arr) => {
            let arr = arr.lock().unwrap();
            let elements_str: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
            Value::String(format!("[{}]", elements_str.join(", ")))
        }
        Value::Promise(_) => Value::String("[promise]".to_string()),
        Value::Exception(e) => Value::String(format!("Exception: {}", e.message)),
    };
    
    Ok(result)
}

pub fn native_str_length(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("length requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        Ok(Value::Int64(s.len() as i64))
    } else {
        Err(Value::String("length requires a string argument".to_string()))
    }
}

pub fn native_str_trim(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("trim requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.trim().to_string()))
    } else {
        Err(Value::String("trim requires a string argument".to_string()))
    }
}

pub fn native_str_split(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("split requires a string and a delimiter".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("split requires a string as the first argument".to_string()));
    };
    
    let delimiter = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("split requires a string as the delimiter".to_string()));
    };
    
    let parts: Vec<String> = input.split(&delimiter).map(|s| s.to_string()).collect();
    let parts_value: Vec<Value> = parts.into_iter().map(Value::String).collect();
    
    Ok(Value::Array(std::sync::Arc::new(std::sync::Mutex::new(parts_value))))
}

pub fn native_str_to_int(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("to_int requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        match s.trim().parse::<i64>() {
            Ok(n) => Ok(Value::Int64(n)),
            Err(_) => Ok(Value::Null),
        }
    } else {
        Err(Value::String("to_int requires a string argument".to_string()))
    }
}

pub fn native_str_to_float(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("to_float requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        match s.trim().parse::<f64>() {
            Ok(n) => Ok(Value::Float64(n)),
            Err(_) => Ok(Value::Null),
        }
    } else {
        Err(Value::String("to_float requires a string argument".to_string()))
    }
}

pub fn native_str_contains(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("contains requires a string and a substring".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("contains requires a string as the first argument".to_string()));
    };
    
    let substring = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("contains requires a string as the substring argument".to_string()));
    };
    
    Ok(Value::Bool(input.contains(&substring)))
}

pub fn native_str_starts_with(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("starts_with requires a string and a prefix".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("starts_with requires a string as the first argument".to_string()));
    };
    
    let prefix = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("starts_with requires a string as the prefix argument".to_string()));
    };
    
    Ok(Value::Bool(input.starts_with(&prefix)))
}

pub fn native_str_ends_with(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("ends_with requires a string and a suffix".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("ends_with requires a string as the first argument".to_string()));
    };
    
    let suffix = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("ends_with requires a string as the suffix argument".to_string()));
    };
    
    Ok(Value::Bool(input.ends_with(&suffix)))
}

pub fn native_str_substring(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("substring requires a string argument".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("substring requires a string as the first argument".to_string()));
    };
    
    let start = if args.len() > 1 {
        args[1].to_int().unwrap_or(0) as usize
    } else {
        0
    };
    
    let end = if args.len() > 2 {
        args[2].to_int().unwrap_or(input.len() as i64) as usize
    } else {
        input.len()
    };
    
    let start = start.min(input.len());
    let end = end.min(input.len());
    
    if start > end {
        return Err(Value::String("substring: start index cannot be greater than end index".to_string()));
    }
    
    Ok(Value::String(input[start..end].to_string()))
}

pub fn native_str_to_lowercase(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("to_lowercase requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.to_lowercase()))
    } else {
        Err(Value::String("to_lowercase requires a string argument".to_string()))
    }
}

pub fn native_str_to_uppercase(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("to_uppercase requires a string argument".to_string()));
    }
    
    if let Value::String(s) = &args[0] {
        Ok(Value::String(s.to_uppercase()))
    } else {
        Err(Value::String("to_uppercase requires a string argument".to_string()))
    }
}

pub fn native_str_replace(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 3 {
        return Err(Value::String("replace requires a string, a pattern, and a replacement".to_string()));
    }
    
    let input = if let Value::String(s) = &args[0] {
        s.clone()
    } else {
        return Err(Value::String("replace requires a string as the first argument".to_string()));
    };
    
    let pattern = if let Value::String(s) = &args[1] {
        s.clone()
    } else {
        return Err(Value::String("replace requires a string as the pattern argument".to_string()));
    };

    let replacement = if let Value::String(s) = &args[2] {
        s.clone()
    } else {
        return Err(Value::String("replace requires a string as the replacement argument".to_string()));
    };

    Ok(Value::String(input.replace(&pattern, &replacement)))
}

/// Native int() function that converts any value to integer
pub fn native_int(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Int64(0));
    }
    
    let result = match &args[0] {
        Value::Int64(n) => Value::Int64(*n),
        Value::Int8(n) => Value::Int64(*n as i64),
        Value::Int16(n) => Value::Int64(*n as i64),
        Value::Int32(n) => Value::Int64(*n as i64),
        Value::UInt8(n) => Value::Int64(*n as i64),
        Value::UInt16(n) => Value::Int64(*n as i64),
        Value::UInt32(n) => Value::Int64(*n as i64),
        Value::UInt64(n) => Value::Int64(*n as i64),
        Value::Float64(f) => Value::Int64(*f as i64),
        Value::Float32(f) => Value::Int64(*f as i64),
        Value::Bool(b) => Value::Int64(if *b { 1 } else { 0 }),
        Value::String(s) => {
            if let Ok(n) = s.parse::<i64>() {
                Value::Int64(n)
            } else if let Ok(f) = s.parse::<f64>() {
                Value::Int64(f as i64)
            } else {
                Value::Int64(0)
            }
        }
        _ => Value::Int64(0),
    };
    
    Ok(result)
}

/// Native float() function that converts any value to float
pub fn native_float(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Float64(0.0));
    }
    
    let result = match &args[0] {
        Value::Int64(n) => Value::Float64(*n as f64),
        Value::Int8(n) => Value::Float64(*n as f64),
        Value::Int16(n) => Value::Float64(*n as f64),
        Value::Int32(n) => Value::Float64(*n as f64),
        Value::UInt8(n) => Value::Float64(*n as f64),
        Value::UInt16(n) => Value::Float64(*n as f64),
        Value::UInt32(n) => Value::Float64(*n as f64),
        Value::UInt64(n) => Value::Float64(*n as f64),
        Value::Float64(f) => Value::Float64(*f),
        Value::Float32(f) => Value::Float64(*f as f64),
        Value::Bool(b) => Value::Float64(if *b { 1.0 } else { 0.0 }),
        Value::String(s) => {
            if let Ok(n) = s.parse::<f64>() {
                Value::Float64(n)
            } else {
                Value::Float64(0.0)
            }
        }
        _ => Value::Float64(0.0),
    };
    
    Ok(result)
}

/// Native bool() function that converts any value to bool
pub fn native_bool(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Bool(false));
    }
    
    let result = match &args[0] {
        Value::Int64(n) => Value::Bool(*n != 0),
        Value::Int8(n) => Value::Bool(*n != 0),
        Value::Int16(n) => Value::Bool(*n != 0),
        Value::Int32(n) => Value::Bool(*n != 0),
        Value::UInt8(n) => Value::Bool(*n != 0),
        Value::UInt16(n) => Value::Bool(*n != 0),
        Value::UInt32(n) => Value::Bool(*n != 0),
        Value::UInt64(n) => Value::Bool(*n != 0),
        Value::Float64(f) => Value::Bool(*f != 0.0),
        Value::Float32(f) => Value::Bool(*f != 0.0),
        Value::Bool(b) => Value::Bool(*b),
        Value::String(s) => Value::Bool(!s.is_empty()),
        Value::Null => Value::Bool(false),
        _ => Value::Bool(true),
    };
    
    Ok(result)
}

/// Native int8() function that converts any value to int8
pub fn native_int8(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Int8(0));
    }
    Ok(Value::Int8(args[0].to_i8().unwrap_or(0)))
}

/// Native uint8() function that converts any value to uint8
pub fn native_uint8(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::UInt8(0));
    }
    Ok(Value::UInt8(args[0].to_u8().unwrap_or(0)))
}

/// Native int16() function that converts any value to int16
pub fn native_int16(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Int16(0));
    }
    Ok(Value::Int16(args[0].to_i16().unwrap_or(0)))
}

/// Native uint16() function that converts any value to uint16
pub fn native_uint16(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::UInt16(0));
    }
    Ok(Value::UInt16(args[0].to_u16().unwrap_or(0)))
}

/// Native int32() function that converts any value to int32
pub fn native_int32(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Int32(0));
    }
    Ok(Value::Int32(args[0].to_i32().unwrap_or(0)))
}

/// Native uint32() function that converts any value to uint32
pub fn native_uint32(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::UInt32(0));
    }
    Ok(Value::UInt32(args[0].to_u32().unwrap_or(0)))
}

/// Native int64() function that converts any value to int64
pub fn native_int64(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Int64(0));
    }
    Ok(Value::Int64(args[0].to_i64().unwrap_or(0)))
}

/// Native uint64() function that converts any value to uint64
pub fn native_uint64(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::UInt64(0));
    }
    Ok(Value::UInt64(args[0].to_u64().unwrap_or(0)))
}

/// Native float32() function that converts any value to float32
pub fn native_float32(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Float32(0.0));
    }
    Ok(Value::Float32(args[0].to_f32().unwrap_or(0.0)))
}

/// Native float64() function that converts any value to float64
pub fn native_float64(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Ok(Value::Float64(0.0));
    }
    Ok(Value::Float64(args[0].to_f64().unwrap_or(0.0)))
}

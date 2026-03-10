use sparkler::Value;

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

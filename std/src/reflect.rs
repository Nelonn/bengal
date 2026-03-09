use std::sync::{Arc, Mutex};
use sparkler::vm::Instance;
use sparkler::Value;

pub fn native_reflect_typeof(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("typeof requires at least one argument".to_string()));
    }
    
    let type_name = match &args[0] {
        Value::String(_) => "string",
        Value::Int8(_) | Value::Int16(_) | Value::Int32(_) | Value::Int64(_) |
        Value::UInt8(_) | Value::UInt16(_) | Value::UInt32(_) | Value::UInt64(_) => "int",
        Value::Float32(_) | Value::Float64(_) => "float",
        Value::Bool(_) => "bool",
        Value::Null => "null",
        Value::Instance(_) => "object",
        Value::Promise(_) => "promise",
        Value::Exception(_) => "exception",
    };
    
    Ok(Value::String(type_name.to_string()))
}

pub fn native_reflect_class_name(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("class_name requires at least one argument".to_string()));
    }
    
    match &args[0] {
        Value::Instance(inst) => Ok(Value::String(inst.lock().unwrap().class.clone())),
        _ => Ok(Value::Null),
    }
}

pub fn native_reflect_fields(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.is_empty() {
        return Err(Value::String("fields requires at least one argument".to_string()));
    }
    
    match &args[0] {
        Value::Instance(inst) => {
            Ok(Value::Instance(Arc::new(Mutex::new(Instance {
                class: "Object".to_string(),
                fields: inst.lock().unwrap().fields.clone(),
            }))))
        }
        _ => Ok(Value::Null),
    }
}

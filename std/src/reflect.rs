use sparkler::{vm::Instance, Value, NativeResult};
use std::sync::{Arc, Mutex};

pub fn native_reflect_typeof(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::String(
            "typeof requires at least one argument".to_string(),
        ));
    }

    let type_name = match &args[0] {
        Value::String(_) => "string",
        Value::Int8(_)
        | Value::Int16(_)
        | Value::Int32(_)
        | Value::Int64(_)
        | Value::UInt8(_)
        | Value::UInt16(_)
        | Value::UInt32(_)
        | Value::UInt64(_) => "int",
        Value::Float32(_) | Value::Float64(_) => "float",
        Value::Bool(_) => "bool",
        Value::Null => "null",
        Value::Instance(_) => "object",
        Value::Array(_) => "array",
        Value::Exception(_) => "exception",
        Value::Promise(_) => "promise",
    };

    NativeResult::Ready(Value::String(type_name.to_string()))
}

pub fn native_reflect_class_name(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::String(
            "class_name requires at least one argument".to_string(),
        ));
    }

    match &args[0] {
        Value::Instance(inst) => NativeResult::Ready(Value::String(inst.lock().unwrap().class.clone())),
        _ => NativeResult::Ready(Value::Null),
    }
}

pub fn native_reflect_fields(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::String(
            "fields requires at least one argument".to_string(),
        ));
    }

    match &args[0] {
        Value::Instance(inst) => {
            let inst_lock = inst.lock().unwrap();
            NativeResult::Ready(Value::Instance(Arc::new(Mutex::new(Instance {
                class: "Object".to_string(),
                fields: inst_lock.fields.clone(),
                private_fields: inst_lock.private_fields.clone(),
                native_data: Arc::new(Mutex::new(None)),
            }))))
        }
        _ => NativeResult::Ready(Value::Null),
    }
}

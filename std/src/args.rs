use sparkler::Value;
use std::sync::{Arc, Mutex};

pub fn native_args_get(_args: &mut Vec<Value>) -> Result<Value, Value> {
    let args: Vec<String> = std::env::args().collect();
    
    // Create an array of strings
    let mut arg_values = Vec::new();
    for arg in args {
        arg_values.push(Value::String(arg));
    }
    
    Ok(Value::Array(Arc::new(Mutex::new(arg_values))))
}

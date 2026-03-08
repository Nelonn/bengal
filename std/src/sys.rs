use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use sparkler::vm::Instance;
use sparkler::Value;

pub fn native_sys_env(args: &mut Vec<Value>) -> Result<Value, String> {
    if !args.is_empty() {
        if let Value::String(key) = &args[0] {
            return match std::env::var(key) {
                Ok(val) => Ok(Value::String(val)),
                Err(_) => Ok(Value::Null),
            };
        } else if let Value::Null = &args[0] {
            // Fall through to returning all env vars
        } else {
            return Err("env requires a string argument or null".to_string());
        }
    }

    let mut env_map = HashMap::new();
    for (key, value) in std::env::vars() {
        env_map.insert(key, Value::String(value));
    }

    Ok(Value::Instance(Arc::new(Mutex::new(Instance {
        class: "Object".to_string(),
        fields: env_map,
    }))))
}

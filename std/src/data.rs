use sparkler::{Value};
use std::sync::{Arc, Mutex};

pub fn native_byte_buffer_native_create(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("ByteBuffer native_create requires instance".to_string()));
    };

    let mut inst = instance.lock().unwrap();
    inst.native_data = Arc::new(Mutex::new(Some(Box::new(Vec::<u8>::new()))));
    
    Ok(Value::Null)
}

pub fn native_byte_buffer_constructor(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() > 1 {
        // constructor(size)
        return native_byte_buffer_reserve(args);
    }
    Ok(Value::Null)
}

pub fn native_byte_buffer_reserve(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("ByteBuffer.reserve requires instance".to_string()));
    };

    let size = args[1].to_int().unwrap_or(0) as usize;
    
    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(buffer) = data.downcast_mut::<Vec<u8>>() {
            *buffer = vec![0u8; size];
            return Ok(Value::Null);
        }
    }
    
    Err(Value::String("ByteBuffer native data not initialized".to_string()))
}

pub fn native_byte_buffer_get(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("ByteBuffer.get requires instance".to_string()));
    };

    let index = args[1].to_int().unwrap_or(0) as usize;
    
    let inst = instance.lock().unwrap();
    let native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_ref() {
        if let Some(buffer) = data.downcast_ref::<Vec<u8>>() {
            if index < buffer.len() {
                return Ok(Value::UInt8(buffer[index]));
            } else {
                return Err(Value::String("ByteBuffer index out of bounds".to_string()));
            }
        }
    }
    
    Err(Value::String("ByteBuffer native data not initialized".to_string()))
}

pub fn native_byte_buffer_set(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("ByteBuffer.set requires instance".to_string()));
    };

    let index = args[1].to_int().unwrap_or(0) as usize;
    let value = args[2].to_u8().unwrap_or(0);
    
    let inst = instance.lock().unwrap();
    let mut native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_mut() {
        if let Some(buffer) = data.downcast_mut::<Vec<u8>>() {
            if index < buffer.len() {
                buffer[index] = value;
                return Ok(Value::Null);
            } else {
                return Err(Value::String("ByteBuffer index out of bounds".to_string()));
            }
        }
    }
    
    Err(Value::String("ByteBuffer native data not initialized".to_string()))
}

pub fn native_byte_buffer_length(args: &mut Vec<Value>) -> Result<Value, Value> {
    let instance = if let Value::Instance(inst) = &args[0] {
        inst.clone()
    } else {
        return Err(Value::String("ByteBuffer.length requires instance".to_string()));
    };

    let inst = instance.lock().unwrap();
    let native_data = inst.native_data.lock().unwrap();
    if let Some(data) = native_data.as_ref() {
        if let Some(buffer) = data.downcast_ref::<Vec<u8>>() {
            return Ok(Value::Int64(buffer.len() as i64));
        }
    }
    
    Ok(Value::Int64(0))
}

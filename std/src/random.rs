use sparkler::Value;
use std::cell::RefCell;
use rand::RngExt;

thread_local! {
    static RNG: RefCell<rand::rngs::ThreadRng> = RefCell::new(rand::rng());
}

pub fn native_random_next_bool(_args: &mut Vec<Value>) -> Result<Value, Value> {
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        Ok(Value::Bool(rng.random_bool(0.5)))
    })
}

pub fn native_random_next_int(_args: &mut Vec<Value>) -> Result<Value, Value> {
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        Ok(Value::Int64(rng.random()))
    })
}

pub fn native_random_next_int_range(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("nextIntRange requires min and max arguments".to_string()));
    }

    let min = match args[0] {
        Value::Int64(v) => v,
        _ => return Err(Value::String("min must be an int".to_string())),
    };
    let max = match args[1] {
        Value::Int64(v) => v,
        _ => return Err(Value::String("max must be an int".to_string())),
    };

    if min >= max {
        return Err(Value::String("min must be less than max".to_string()));
    }

    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        Ok(Value::Int64(rng.random_range(min..max)))
    })
}

pub fn native_random_next_float(_args: &mut Vec<Value>) -> Result<Value, Value> {
    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        Ok(Value::Float64(rng.random()))
    })
}

pub fn native_random_next_float_range(args: &mut Vec<Value>) -> Result<Value, Value> {
    if args.len() < 2 {
        return Err(Value::String("nextFloatRange requires min and max arguments".to_string()));
    }

    let min = match args[0] {
        Value::Float64(v) => v,
        _ => return Err(Value::String("min must be a float".to_string())),
    };
    let max = match args[1] {
        Value::Float64(v) => v,
        _ => return Err(Value::String("max must be a float".to_string())),
    };

    if min >= max {
        return Err(Value::String("min must be less than max".to_string()));
    }

    RNG.with(|rng| {
        let mut rng = rng.borrow_mut();
        Ok(Value::Float64(rng.random_range(min..max)))
    })
}

use sparkler::Value;

pub fn native_print(args: &mut Vec<Value>) -> Result<Value, Value> {
    for arg in args {
        print!("{}", arg.to_string());
    }
    Ok(Value::Null)
}

pub fn native_println(args: &mut Vec<Value>) -> Result<Value, Value> {
    for arg in args {
        print!("{}", arg.to_string());
    }
    println!();
    Ok(Value::Null)
}

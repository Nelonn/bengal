use sparkler::{Value, NativeResult};

pub fn native_print(args: &mut Vec<Value>) -> NativeResult {
    for arg in args {
        print!("{}", arg.to_string());
    }
    NativeResult::Ready(Value::Null)
}

pub fn native_println(args: &mut Vec<Value>) -> NativeResult {
    for arg in args {
        print!("{}", arg.to_string());
    }
    println!();
    NativeResult::Ready(Value::Null)
}

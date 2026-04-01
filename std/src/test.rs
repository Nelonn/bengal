use sparkler::{Value, NativeResult, NativeContext};

/// Assert that two values are the same type and equal
pub fn native_assert_same(_ctx: &NativeContext, args: &mut Vec<Value>) -> NativeResult {
    if args.len() < 2 {
        return NativeResult::Ready(Value::String("assertSame requires expected and actual values".to_string()));
    }

    let expected = &args[0];
    let actual = &args[1];

    let message = if args.len() > 2 {
        if let Value::String(s) = &args[2] {
            s.clone()
        } else {
            format!("Expected same value {:?}, got {:?}", expected, actual)
        }
    } else {
        format!("Expected same value {:?}, got {:?}", expected, actual)
    };

    // Check both type and value
    let same = match (expected, actual) {
        (Value::Int64(a), Value::Int64(b)) => a == b,
        (Value::Int32(a), Value::Int32(b)) => a == b,
        (Value::Int16(a), Value::Int16(b)) => a == b,
        (Value::Int8(a), Value::Int8(b)) => a == b,
        (Value::UInt64(a), Value::UInt64(b)) => a == b,
        (Value::UInt32(a), Value::UInt32(b)) => a == b,
        (Value::UInt16(a), Value::UInt16(b)) => a == b,
        (Value::UInt8(a), Value::UInt8(b)) => a == b,
        (Value::Float64(a), Value::Float64(b)) => a == b,
        (Value::Float32(a), Value::Float32(b)) => a == b,
        (Value::String(a), Value::String(b)) => a == b,
        (Value::Bool(a), Value::Bool(b)) => a == b,
        (Value::Null, Value::Null) => true,
        _ => false,
    };

    if !same {
        NativeResult::Ready(Value::String(message))
    } else {
        NativeResult::Ready(Value::Null)
    }
}

/// Fail the test immediately with a message
pub fn native_fail(_ctx: &NativeContext, args: &mut Vec<Value>) -> NativeResult {
    let message = if !args.is_empty() {
        if let Value::String(s) = &args[0] {
            s.clone()
        } else {
            "Test failed".to_string()
        }
    } else {
        "Test failed".to_string()
    };

    NativeResult::Ready(Value::String(message))
}

/// Set current test name (called by test() function)
pub fn native_set_current_test(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    NativeResult::Ready(Value::Null)
}

/// Record a passing test
pub fn native_record_pass(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    NativeResult::Ready(Value::Null)
}

use sparkler::{Value, NativeResult};

fn get_float(args: &mut Vec<Value>, index: usize) -> f64 {
    args[index].to_float().unwrap_or(0.0)
}

// Trigonometric functions
pub fn native_math_sin(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.sin()))
}

pub fn native_math_cos(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.cos()))
}

pub fn native_math_tan(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.tan()))
}

pub fn native_math_asin(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.asin()))
}

pub fn native_math_acos(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.acos()))
}

pub fn native_math_atan(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.atan()))
}

pub fn native_math_atan2(args: &mut Vec<Value>) -> NativeResult {
    let y = get_float(args, 0);
    let x = get_float(args, 1);
    NativeResult::Ready(Value::Float64(y.atan2(x)))
}

// Hyperbolic functions
pub fn native_math_sinh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.sinh()))
}

pub fn native_math_cosh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.cosh()))
}

pub fn native_math_tanh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.tanh()))
}

pub fn native_math_asinh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.asinh()))
}

pub fn native_math_acosh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.acosh()))
}

pub fn native_math_atanh(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.atanh()))
}

// Rounding functions
pub fn native_math_floor(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.floor()))
}

pub fn native_math_ceil(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.ceil()))
}

pub fn native_math_round(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.round()))
}

pub fn native_math_trunc(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.trunc()))
}

pub fn native_math_fract(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.fract()))
}

// Comparison functions
pub fn native_math_min(args: &mut Vec<Value>) -> NativeResult {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    NativeResult::Ready(Value::Float64(a.min(b)))
}

pub fn native_math_max(args: &mut Vec<Value>) -> NativeResult {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    NativeResult::Ready(Value::Float64(a.max(b)))
}

pub fn native_math_clamp(args: &mut Vec<Value>) -> NativeResult {
    let value = get_float(args, 0);
    let min = get_float(args, 1);
    let max = get_float(args, 2);
    NativeResult::Ready(Value::Float64(value.clamp(min, max)))
}

pub fn native_math_abs(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.abs()))
}

pub fn native_math_sign(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    let sign = if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 };
    NativeResult::Ready(Value::Float64(sign))
}

// Power and root functions
pub fn native_math_sqrt(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.sqrt()))
}

pub fn native_math_cbrt(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.cbrt()))
}

pub fn native_math_pow(args: &mut Vec<Value>) -> NativeResult {
    let base = get_float(args, 0);
    let exp = get_float(args, 1);
    NativeResult::Ready(Value::Float64(base.powf(exp)))
}

pub fn native_math_exp(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.exp()))
}

pub fn native_math_ln(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.ln()))
}

pub fn native_math_log10(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.log10()))
}

pub fn native_math_log2(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    NativeResult::Ready(Value::Float64(x.log2()))
}

pub fn native_math_log(args: &mut Vec<Value>) -> NativeResult {
    let base = get_float(args, 0);
    let x = get_float(args, 1);
    NativeResult::Ready(Value::Float64(x.log(base)))
}

// Utility functions
pub fn native_math_hypot(args: &mut Vec<Value>) -> NativeResult {
    let x = get_float(args, 0);
    let y = get_float(args, 1);
    NativeResult::Ready(Value::Float64(x.hypot(y)))
}

pub fn native_math_lerp(args: &mut Vec<Value>) -> NativeResult {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    let t = get_float(args, 2);
    NativeResult::Ready(Value::Float64(a + (b - a) * t))
}

pub fn native_math_step(args: &mut Vec<Value>) -> NativeResult {
    let edge = get_float(args, 0);
    let x = get_float(args, 1);
    NativeResult::Ready(Value::Float64(if x < edge { 0.0 } else { 1.0 }))
}

pub fn native_math_smoothstep(args: &mut Vec<Value>) -> NativeResult {
    let edge0 = get_float(args, 0);
    let edge1 = get_float(args, 1);
    let x = get_float(args, 2);
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    NativeResult::Ready(Value::Float64(t * t * (3.0 - 2.0 * t)))
}

// Angle conversion
pub fn native_math_to_radians(args: &mut Vec<Value>) -> NativeResult {
    let degrees = get_float(args, 0);
    NativeResult::Ready(Value::Float64(degrees.to_radians()))
}

pub fn native_math_to_degrees(args: &mut Vec<Value>) -> NativeResult {
    let radians = get_float(args, 0);
    NativeResult::Ready(Value::Float64(radians.to_degrees()))
}

pub fn native_math_check_overflow(args: &mut Vec<Value>) -> NativeResult {
    if args.len() < 5 {
        return NativeResult::Ready(Value::Null);
    }

    let a = match args[0] {
        Value::Int64(v) => v,
        _ => return NativeResult::Ready(Value::Null),
    };
    let b = match args[1] {
        Value::Int64(v) => v,
        _ => return NativeResult::Ready(Value::Null),
    };
    let res = match args[2] {
        Value::Int64(v) => v,
        _ => return NativeResult::Ready(Value::Null),
    };
    let op = match args[3] {
        Value::Int64(v) => v, // 0: Add, 1: Sub, 2: Mul
        _ => return NativeResult::Ready(Value::Null),
    };
    // Type: 1: int8, 2: uint8, 3: int16, 4: uint16, 5: int32, 6: uint32, 7: int64, 8: uint64, 0: int (no bounds check needed)
    let type_id = match args[4] {
        Value::Int64(v) => v,
        _ => return NativeResult::Ready(Value::Null),
    };

    // First check if the operation itself overflowed i64
    let base_overflow = match op {
        0 => { // Add
            let (sum, overflow) = a.overflowing_add(b);
            overflow || sum != res
        }
        1 => { // Sub
            let (diff, overflow) = a.overflowing_sub(b);
            overflow || diff != res
        }
        2 => { // Mul
            let (prod, overflow) = a.overflowing_mul(b);
            overflow || prod != res
        }
        _ => false,
    };

    if base_overflow {
        return NativeResult::Ready(Value::String("Integer overflow".to_string()));
    }

    // Now check if the result fits in the target type
    let in_range = match type_id {
        1 => res >= -128 && res <= 127, // int8
        2 => res >= 0 && res <= 255, // uint8
        3 => res >= -32768 && res <= 32767, // int16
        4 => res >= 0 && res <= 65535, // uint16
        5 => res >= -2147483648 && res <= 2147483647, // int32
        6 => res >= 0 && res <= 4294967295, // uint32
        7 => true, // int64 - always in range
        8 => res >= 0, // uint64 - must be non-negative
        _ => true, // int or unknown - no bounds check
    };

    if !in_range {
        return NativeResult::Ready(Value::String("Integer overflow".to_string()));
    }

    NativeResult::Ready(Value::Null)
}

pub fn native_math_check_div_zero(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::Null);
    }

    let is_zero = match args[0] {
        Value::Int64(v) => v == 0,
        Value::Float64(v) => v == 0.0,
        _ => false,
    };

    if is_zero {
        return NativeResult::Ready(Value::String("Division by zero".to_string()));
    }

    NativeResult::Ready(Value::Null)
}

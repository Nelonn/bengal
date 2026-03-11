use sparkler::Value;

fn get_float(args: &mut Vec<Value>, index: usize) -> f64 {
    args[index].to_float().unwrap_or(0.0)
}

// Trigonometric functions
pub fn native_math_sin(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.sin()))
}

pub fn native_math_cos(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.cos()))
}

pub fn native_math_tan(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.tan()))
}

pub fn native_math_asin(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.asin()))
}

pub fn native_math_acos(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.acos()))
}

pub fn native_math_atan(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.atan()))
}

pub fn native_math_atan2(args: &mut Vec<Value>) -> Result<Value, Value> {
    let y = get_float(args, 0);
    let x = get_float(args, 1);
    Ok(Value::Float64(y.atan2(x)))
}

// Hyperbolic functions
pub fn native_math_sinh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.sinh()))
}

pub fn native_math_cosh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.cosh()))
}

pub fn native_math_tanh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.tanh()))
}

pub fn native_math_asinh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.asinh()))
}

pub fn native_math_acosh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.acosh()))
}

pub fn native_math_atanh(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.atanh()))
}

// Rounding functions
pub fn native_math_floor(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.floor()))
}

pub fn native_math_ceil(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.ceil()))
}

pub fn native_math_round(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.round()))
}

pub fn native_math_trunc(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.trunc()))
}

pub fn native_math_fract(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.fract()))
}

// Comparison functions
pub fn native_math_min(args: &mut Vec<Value>) -> Result<Value, Value> {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    Ok(Value::Float64(a.min(b)))
}

pub fn native_math_max(args: &mut Vec<Value>) -> Result<Value, Value> {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    Ok(Value::Float64(a.max(b)))
}

pub fn native_math_clamp(args: &mut Vec<Value>) -> Result<Value, Value> {
    let value = get_float(args, 0);
    let min = get_float(args, 1);
    let max = get_float(args, 2);
    Ok(Value::Float64(value.clamp(min, max)))
}

pub fn native_math_abs(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.abs()))
}

pub fn native_math_sign(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    let sign = if x > 0.0 { 1.0 } else if x < 0.0 { -1.0 } else { 0.0 };
    Ok(Value::Float64(sign))
}

// Power and root functions
pub fn native_math_sqrt(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.sqrt()))
}

pub fn native_math_cbrt(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.cbrt()))
}

pub fn native_math_pow(args: &mut Vec<Value>) -> Result<Value, Value> {
    let base = get_float(args, 0);
    let exp = get_float(args, 1);
    Ok(Value::Float64(base.powf(exp)))
}

pub fn native_math_exp(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.exp()))
}

pub fn native_math_ln(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.ln()))
}

pub fn native_math_log10(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.log10()))
}

pub fn native_math_log2(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    Ok(Value::Float64(x.log2()))
}

pub fn native_math_log(args: &mut Vec<Value>) -> Result<Value, Value> {
    let base = get_float(args, 0);
    let x = get_float(args, 1);
    Ok(Value::Float64(x.log(base)))
}

// Utility functions
pub fn native_math_hypot(args: &mut Vec<Value>) -> Result<Value, Value> {
    let x = get_float(args, 0);
    let y = get_float(args, 1);
    Ok(Value::Float64(x.hypot(y)))
}

pub fn native_math_lerp(args: &mut Vec<Value>) -> Result<Value, Value> {
    let a = get_float(args, 0);
    let b = get_float(args, 1);
    let t = get_float(args, 2);
    Ok(Value::Float64(a + (b - a) * t))
}

pub fn native_math_step(args: &mut Vec<Value>) -> Result<Value, Value> {
    let edge = get_float(args, 0);
    let x = get_float(args, 1);
    Ok(Value::Float64(if x < edge { 0.0 } else { 1.0 }))
}

pub fn native_math_smoothstep(args: &mut Vec<Value>) -> Result<Value, Value> {
    let edge0 = get_float(args, 0);
    let edge1 = get_float(args, 1);
    let x = get_float(args, 2);
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    Ok(Value::Float64(t * t * (3.0 - 2.0 * t)))
}

// Angle conversion
pub fn native_math_to_radians(args: &mut Vec<Value>) -> Result<Value, Value> {
    let degrees = get_float(args, 0);
    Ok(Value::Float64(degrees.to_radians()))
}

pub fn native_math_to_degrees(args: &mut Vec<Value>) -> Result<Value, Value> {
    let radians = get_float(args, 0);
    Ok(Value::Float64(radians.to_degrees()))
}

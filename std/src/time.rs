use sparkler::{Value, NativeResult, NativeContext};
use std::time::{SystemTime, UNIX_EPOCH};
use chrono::Timelike;

pub fn native_time_current_time(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");

    let seconds = duration.as_secs() as f64 + duration.subsec_nanos() as f64 / 1_000_000_000.0;
    NativeResult::Ready(Value::Float64(seconds))
}

pub fn native_time_current_hour(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    let datetime = chrono::Local::now();
    NativeResult::Ready(Value::Int64(datetime.hour() as i64))
}

pub fn native_time_current_min(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    let datetime = chrono::Local::now();
    NativeResult::Ready(Value::Int64(datetime.minute() as i64))
}

pub fn native_time_current_sec(_ctx: &NativeContext, _args: &mut Vec<Value>) -> NativeResult {
    let datetime = chrono::Local::now();
    NativeResult::Ready(Value::Int64(datetime.second() as i64))
}

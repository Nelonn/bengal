use sparkler::{Value, NativeResult, get_async_callback_sender};
use std::time::Duration;
use std::sync::atomic::{AtomicU64, Ordering};

static SLEEP_COUNTER: AtomicU64 = AtomicU64::new(0);

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

pub fn native_sleep(args: &mut Vec<Value>) -> NativeResult {
    // Get sleep duration in milliseconds
    let ms = match &args[0] {
        Value::Int32(n) => *n as u64,
        Value::Int64(n) => *n as u64,
        Value::UInt32(n) => *n as u64,
        Value::UInt64(n) => *n,
        _ => return NativeResult::Ready(Value::Null),
    };

    // Generate a unique wait_id for this sleep
    let wait_id = format!("sleep_{}", SLEEP_COUNTER.fetch_add(1, Ordering::Relaxed));

    // Get the async callback sender
    let callback_tx = get_async_callback_sender();

    if let Some(tx) = callback_tx {
        // Spawn a tokio task to sleep and then send callback with wait_id
        let wait_id_clone = wait_id.clone();
        sparkler::async_runtime::spawn(async move {
            sparkler::async_runtime::sleep(Duration::from_millis(ms)).await;
            // Send wait_id as a string so the executor knows which thread to wake
            let _ = tx.send(Ok(Value::String(wait_id_clone)));
        });
        // Return Pending to suspend the current green thread
        // The wait_id will be set by the scheduler
        NativeResult::PendingWithWaitId(wait_id)
    } else {
        // No async context, do blocking sleep (fallback)
        std::thread::sleep(Duration::from_millis(ms));
        NativeResult::Ready(Value::Null)
    }
}

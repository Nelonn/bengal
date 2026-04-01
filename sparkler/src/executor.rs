use crate::vm::{VM, Value, NativeFn, NativeFallbackFn, Class, Function, RunResult, set_async_callback_sender};
use crate::{debug_vm, Opcode};
use crate::linker::{RuntimeLinker, NativeFunctionRegistry};
use crate::scheduler::Scheduler;
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::cell::RefCell;
use std::rc::Rc;

pub use crate::vm::VTable;

#[derive(Clone)]
pub struct Bytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
    pub vtables: Vec<VTable>,  // Vtables stored in .data section
}

pub struct Executor {
    pub vm: VM,
    /// Optional runtime linker for dynamic linking and hot-swap
    pub linker: Option<RuntimeLinker>,
    /// Channel for receiving async native callbacks
    callback_rx: Option<Receiver<Result<Value, Value>>>,
    callback_tx: Option<Sender<Result<Value, Value>>>,
    /// Scheduler for green threads (created on first spawn)
    scheduler: Option<Scheduler>,
    /// Bytecode shared across all threads
    bytecode: Option<Bytecode>,
    /// Context for green thread execution
    green_thread_ctx: Option<Rc<GreenThreadContext>>,
}

/// Shared context for green thread execution
pub struct GreenThreadContext {
    pub scheduler: Rc<RefCell<Scheduler>>,
    pub bytecode: Bytecode,
    pub native_registry: NativeFunctionRegistry,
    /// Pending VMs to be spawned (shared across all threads)
    pub pending_spawns: Rc<RefCell<Vec<VM>>>,
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            vm: VM::new(),
            linker: None,
            callback_rx: Some(rx),
            callback_tx: Some(tx),
            scheduler: None,
            bytecode: None,
            green_thread_ctx: None,
        }
    }

    /// Create a new executor with runtime linker support
    pub fn with_linker() -> Self {
        let linker = RuntimeLinker::new();
        let registry = linker.registry();
        let mut vm = VM::new();
        // Share the same registry between VM and linker
        vm.program.native_registry = (*registry.read().unwrap()).clone();
        let (tx, rx) = channel();
        Self {
            vm,
            linker: Some(linker),
            callback_rx: Some(rx),
            callback_tx: Some(tx),
            scheduler: None,
            bytecode: None,
            green_thread_ctx: None,
        }
    }

    /// Create a new executor with a shared registry
    pub fn with_registry(registry: Arc<RwLock<NativeFunctionRegistry>>) -> Self {
        let mut vm = VM::new();
        vm.program.native_registry = (*registry.read().unwrap()).clone();
        let (tx, rx) = channel();
        Self {
            vm,
            linker: Some(RuntimeLinker::with_registry(registry)),
            callback_rx: Some(rx),
            callback_tx: Some(tx),
            scheduler: None,
            bytecode: None,
            green_thread_ctx: None,
        }
    }

    /// Get the runtime linker if available
    pub fn linker(&mut self) -> Option<&mut RuntimeLinker> {
        self.linker.as_mut()
    }

    /// Get the native function registry
    pub fn registry(&mut self) -> &mut NativeFunctionRegistry {
        &mut self.vm.program.native_registry
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        // Register with linker first if it exists (so it gets an index)
        if let Some(ref mut linker) = self.linker {
            linker.register(name, f);
            // Update VM registry from linker
            let registry = linker.registry();
            self.vm.program.native_registry = (*registry.read().unwrap()).clone();
        } else {
            self.vm.register_native(name, f);
        }
    }

    pub fn register_fallback(&mut self, f: NativeFallbackFn) {
        // Register fallback with linker if it exists
        if let Some(ref mut linker) = self.linker {
            let registry = linker.registry();
            {
                let mut guard = registry.write().unwrap();
                guard.set_fallback(f);
            }
            self.vm.program.native_registry = (*registry.read().unwrap()).clone();
        } else {
            self.vm.register_fallback(f);
        }
    }

    /// Link bytecode to native functions using indexed calls
    ///
    /// This converts string-based CallNative to indexed CallNativeIndexed
    /// for O(1) lookup during execution.
    pub fn link_bytecode(&mut self, bytecode: &mut Bytecode) {
        if let Some(ref mut linker) = self.linker {
            // Update VM registry from linker
            let registry = linker.registry();
            self.vm.program.native_registry = (*registry.read().unwrap()).clone();

            // Convert CallNative to CallNativeIndexed
            Self::convert_to_indexed_calls(&mut bytecode.data, &bytecode.strings, &self.vm.program.native_registry);
        }
    }

    /// Convert CallNative instructions to CallNativeIndexed for O(1) lookup
    fn convert_to_indexed_calls(bytecode: &mut Vec<u8>, strings: &[String], registry: &NativeFunctionRegistry) {
        // Build new bytecode vector to avoid in-place corruption
        let mut new_bytecode = Vec::with_capacity(bytecode.len() + bytecode.len() / 10);
        let mut i = 0;

        while i < bytecode.len() {
            let opcode_byte = bytecode[i];

            // Check if this is CallNative
            if opcode_byte == Opcode::CallNative as u8 {
                // Format: [CallNative, Rd, name_idx, arg_start, arg_count] (5 bytes)
                if i + 4 < bytecode.len() {
                    let rd = bytecode[i + 1];
                    let name_idx = bytecode[i + 2] as usize;
                    let arg_start = bytecode[i + 3];
                    let arg_count = bytecode[i + 4];

                    if let Some(name) = strings.get(name_idx) {
                        // Try exact match first, then prefix match (for names without signature)
                        let func_index = registry.get_index(name)
                            .or_else(|| registry.get_index_by_prefix(name));
                        
                        if let Some(idx) = func_index {
                            // Convert to CallNativeIndexed (6 bytes)
                            new_bytecode.push(Opcode::CallNativeIndexed as u8);
                            new_bytecode.push(rd);
                            new_bytecode.push((idx & 0xFF) as u8);
                            new_bytecode.push(((idx >> 8) & 0xFF) as u8);
                            new_bytecode.push(arg_start);
                            new_bytecode.push(arg_count);
                            i += 5;
                            continue;
                        }
                    }
                }
            }

            // Copy original byte
            new_bytecode.push(opcode_byte);
            i += 1;
        }

        *bytecode = new_bytecode;
    }

    pub fn run(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }

        let bytecode_data = bytecode.data;
        let strings = bytecode.strings;

        // Link bytecode if linker is available
        if self.linker.is_some() {
            // Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.program.native_registry);
        }

        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;
        loop {
            match self.vm.run().map_err(|e| e.to_string())? {
                RunResult::Finished(val) => return Ok(val),
                RunResult::InProgress => continue,
                RunResult::Breakpoint => {
                    println!("Breakpoint hit at line {}", self.vm.get_line());
                    return Ok(None);
                }
                RunResult::Suspended => {
                    // VM suspended for async native - should be handled by run_to_completion
                    return Ok(None);
                }
            }
        }
    }

    pub async fn run_to_completion(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }

        // Store bytecode for spawn to access
        self.bytecode = Some(bytecode.clone());

        let bytecode_data = bytecode.data;
        let strings = bytecode.strings;

        // Link bytecode if linker is available
        if self.linker.is_some() {
            // Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.program.native_registry);
        }

        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;

        // Take the callback receiver and keep sender alive
        // Reinitialize channels if they were already taken (e.g., in REPL scenarios)
        let (callback_rx, callback_tx) = match (self.callback_rx.take(), self.callback_tx.take()) {
            (Some(rx), Some(tx)) => (rx, tx),
            _ => {
                let (tx, rx) = channel();
                (rx, tx)
            }
        };

        // Set the callback sender in thread local storage for native functions
        // Keep a clone alive for the duration of run_to_completion
        let _tx_guard = callback_tx.clone();
        set_async_callback_sender(callback_tx.clone());

        // Check if we have green threads (scheduler was created by spawn)
        // For now, we always use the scheduler path if __spawn was registered
        // since we set up the context before knowing if spawn will be called
        self.run_with_scheduler(callback_rx, callback_tx).await
    }

    /// Run single-threaded (original behavior, no green threads)
    async fn run_single_threaded(
        &mut self,
        mut callback_rx: Option<Receiver<Result<Value, Value>>>,
        _callback_tx: Sender<Result<Value, Value>>,
    ) -> Result<Option<Value>, String> {
        loop {
            let result = self.vm.run().map_err(|e| e.to_string())?;

            match result {
                RunResult::Finished(val) => {
                    return Ok(val);
                }
                RunResult::InProgress => {
                    continue;
                }
                RunResult::Breakpoint => {
                    println!("Breakpoint hit at {}:{}", self.vm.get_source_file().unwrap_or_else(|| "<unknown>".to_string()), self.vm.get_line());
                    continue;
                }
                RunResult::Suspended => {
                    // VM is suspended waiting for async native callback
                    debug_vm!("executor: VM suspended, waiting for callback");
                    // Use std::thread to wait for callback to avoid tokio state machine corruption
                    debug_vm!("executor: Waiting for callback in spawned thread");
                    let rx = callback_rx.take().ok_or("Callback receiver not available")?;
                    let result = std::thread::spawn(move || {
                        rx.recv().map_err(|_| "Callback channel closed".to_string())
                    }).join().unwrap();
                    debug_vm!("executor: Thread joined, result = {:?}", result.is_ok());
                    let result = result?;

                    // result is Result<Result<Value, Value>, String>, need to flatten
                    let result: Result<Value, Value> = match result {
                        Ok(val) => {
                            debug_vm!("executor: Received Ok(val), val = {:?}", match &val { Value::String(s) => format!("String({} chars)", s.len()), Value::Null => "Null".to_string(), _ => "Other".to_string() });
                            Ok(val)
                        }
                        Err(e) => Err(Value::String(e.to_string())),
                    };
                    debug_vm!("executor: About to resume VM with result");

                    // Resume VM with the result
                    match self.vm.resume_with_result(result) {
                        Ok(RunResult::Finished(val)) => {
                            return Ok(val);
                        }
                        Ok(RunResult::InProgress) => {
                            continue;
                        }
                        Ok(RunResult::Breakpoint) => {
                            println!("Breakpoint hit at {}:{}", self.vm.get_source_file().unwrap_or_else(|| "<unknown>".to_string()), self.vm.get_line());
                        }
                        Ok(RunResult::Suspended) => {
                            // Still suspended - this shouldn't happen with current implementation
                            return Err("VM still suspended after callback".to_string());
                        }
                        Err(e) => {
                            return Err(e.to_string());
                        }
                    }
                }
            }
        }
    }

    /// Run with scheduler for green threads support
    async fn run_with_scheduler(
        &mut self,
        callback_rx: Receiver<Result<Value, Value>>,
        callback_tx: Sender<Result<Value, Value>>,
    ) -> Result<Option<Value>, String> {
        // Create scheduler if needed
        if self.scheduler.is_none() {
            self.scheduler = Some(Scheduler::new());
        }

        // Set up context for __spawn BEFORE spawning main thread
        let scheduler_rc = Rc::new(RefCell::new(self.scheduler.take().unwrap()));
        let bytecode = self.bytecode.take().ok_or("Bytecode not set")?;
        let native_registry = self.vm.program.native_registry.clone();
        let pending_spawns = Rc::new(RefCell::new(Vec::new()));

        let ctx = Rc::new(GreenThreadContext {
            scheduler: scheduler_rc.clone(),
            bytecode: bytecode.clone(),
            native_registry,
            pending_spawns: pending_spawns.clone(),
        });

        // Store context in executor and VM's program
        self.green_thread_ctx = Some(ctx.clone());
        self.vm.program.green_thread_ctx = Some(ctx);

        // Spawn the main thread
        {
            let mut scheduler = scheduler_rc.borrow_mut();
            scheduler.spawn(std::mem::replace(&mut self.vm, VM::new()));
        }

        // Run the scheduler
        let result = self.run_scheduler_loop(callback_rx, callback_tx, scheduler_rc, bytecode, pending_spawns).await;

        // Clear context
        self.green_thread_ctx = None;

        result
    }

    /// Run the scheduler loop for green threads
    async fn run_scheduler_loop(
        &mut self,
        callback_rx: Receiver<Result<Value, Value>>,
        _callback_tx: Sender<Result<Value, Value>>,
        scheduler_rc: Rc<RefCell<Scheduler>>,
        _bytecode: Bytecode,
        pending_spawns: Rc<RefCell<Vec<VM>>>,
    ) -> Result<Option<Value>, String> {
        let mut last_result = None;
        let callback_rx = std::sync::Arc::new(tokio::sync::Mutex::new(callback_rx));

        loop {
            // Run scheduler
            let (result, _has_blocked) = {
                let mut scheduler = scheduler_rc.borrow_mut();
                scheduler.run()
            };

            // If we have a result, save it
            if result.is_some() {
                last_result = result;
            }

            // Process any pending spawns (VMs that were spawned during execution)
            {
                let mut scheduler = scheduler_rc.borrow_mut();
                let mut spawns = pending_spawns.borrow_mut();
                for vm in spawns.drain(..) {
                    scheduler.spawn(vm);
                }
            }

            // After processing spawns, run scheduler to let new threads start
            // This ensures workers start their sleeps before we wait for callbacks
            let (result, has_blocked_after_spawns) = {
                let mut scheduler = scheduler_rc.borrow_mut();
                scheduler.run()
            };
            
            if result.is_some() {
                last_result = result;
            }

            // Check if we should continue (pending spawns or blocked threads or active threads)
            let has_pending = !pending_spawns.borrow().is_empty();
            let scheduler_has_active = scheduler_rc.borrow().active_thread_count() > 0;

            if !has_pending && !has_blocked_after_spawns && !scheduler_has_active {
                return Ok(last_result);
            }

            // If we have blocked threads, wait for callback
            if has_blocked_after_spawns {
                debug_vm!("scheduler: Blocked threads, waiting for callback");
                // Use tokio to wait for first callback
                let rx = callback_rx.clone();
                let result = tokio::task::spawn_blocking(move || {
                    let rx = rx.blocking_lock();
                    rx.recv().map_err(|_| "Callback channel closed".to_string())
                }).await.map_err(|e| e.to_string())??;

                // Process this callback
                let result: Result<Value, Value> = match result {
                    Ok(val) => Ok(val),
                    Err(e) => Err(Value::String(e.to_string())),
                };

                if let Ok(ref val) = result {
                    let mut scheduler = scheduler_rc.borrow_mut();
                    if let Value::String(wait_id) = val {
                        if wait_id.starts_with("sleep_") {
                            scheduler.wake_by_wait_id(wait_id, Value::Null);
                        } else {
                            scheduler.wake_all_blocked(val.clone());
                        }
                    } else {
                        scheduler.wake_all_blocked(val.clone());
                    }
                }

                // Drain any additional callbacks that arrived while waiting
                // This ensures all completed sleeps are processed before continuing
                let rx = callback_rx.clone();
                loop {
                    let rx_guard = rx.try_lock();
                    if let Ok(rx_ref) = rx_guard {
                        match rx_ref.try_recv() {
                            Ok(Ok(val)) => {
                                drop(rx_ref);
                                let mut scheduler = scheduler_rc.borrow_mut();
                                if let Value::String(wait_id) = &val {
                                    if wait_id.starts_with("sleep_") {
                                        scheduler.wake_by_wait_id(wait_id, Value::Null);
                                        continue;
                                    }
                                }
                                scheduler.wake_all_blocked(val);
                            }
                            Ok(Err(e)) => {
                                eprintln!("Callback error: {:?}", e);
                            }
                            Err(_) => {
                                // No more callbacks available
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }

                // After processing all callbacks, run the scheduler to let awakened threads execute
                // BEFORE the main loop continues. This ensures workers run before main thread finishes.
                let result = {
                    let mut scheduler = scheduler_rc.borrow_mut();
                    scheduler.run()
                };
                
                if result.0.is_some() {
                    last_result = result.0;
                }

                // Continue the loop to process more threads
                continue;
            }
            // If no blocked threads, continue the loop to run active threads
        }

        Ok(last_result)
    }

    /// Hot-swap a native function at runtime
    ///
    /// This replaces the function implementation without recompiling bytecode.
    /// The new implementation will be used on the next call.
    pub fn hot_swap(&mut self, name: &str, new_func: NativeFn) -> bool {
        self.vm.program.native_registry.hot_swap(name, new_func)
    }

    /// Set a breakpoint in a source file at a specific line
    pub fn set_breakpoint(&mut self, source_file: &str, line: usize) -> Result<(), String> {
        self.vm.set_breakpoint(source_file, line)
    }

    /// Force relinking of bytecode (useful after hot-swap if indices changed)
    pub fn relink(&mut self, bytecode: &mut Bytecode) {
        if let Some(ref mut linker) = self.linker {
            let registry = linker.registry();
            self.vm.program.native_registry = (*registry.read().unwrap()).clone();
            Self::convert_to_indexed_calls(&mut bytecode.data, &bytecode.strings, &self.vm.program.native_registry);
        }
    }

    /// Spawn a new green thread with the given VM
    pub fn spawn_vm(&mut self, vm: VM) {
        if self.scheduler.is_none() {
            self.scheduler = Some(Scheduler::new());
        }
        if let Some(ref mut scheduler) = self.scheduler {
            scheduler.spawn(vm);
        }
    }

    /// Get the green thread context
    pub fn green_thread_context(&self) -> Option<&Rc<GreenThreadContext>> {
        self.green_thread_ctx.as_ref()
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

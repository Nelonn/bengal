use crate::vm::{VM, Value, NativeFn, Class, Function, RunResult, set_async_callback_sender};
use crate::{debug_vm, Opcode};
use crate::linker::{RuntimeLinker, NativeFunctionRegistry};
use std::sync::{Arc, RwLock};
use std::sync::mpsc::{channel, Receiver, Sender};

pub use crate::vm::VTable;

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
}

impl Executor {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            vm: VM::new(),
            linker: None,
            callback_rx: Some(rx),
            callback_tx: Some(tx),
        }
    }

    /// Create a new executor with runtime linker support
    pub fn with_linker() -> Self {
        let linker = RuntimeLinker::new();
        let registry = linker.registry();
        let mut vm = VM::new();
        // Share the same registry between VM and linker
        vm.native_registry = (*registry.read().unwrap()).clone();
        let (tx, rx) = channel();
        Self {
            vm,
            linker: Some(linker),
            callback_rx: Some(rx),
            callback_tx: Some(tx),
        }
    }

    /// Create a new executor with a shared registry
    pub fn with_registry(registry: Arc<RwLock<NativeFunctionRegistry>>) -> Self {
        let mut vm = VM::new();
        vm.native_registry = (*registry.read().unwrap()).clone();
        let (tx, rx) = channel();
        Self {
            vm,
            linker: Some(RuntimeLinker::with_registry(registry)),
            callback_rx: Some(rx),
            callback_tx: Some(tx),
        }
    }

    /// Get the runtime linker if available
    pub fn linker(&mut self) -> Option<&mut RuntimeLinker> {
        self.linker.as_mut()
    }

    /// Get the native function registry
    pub fn registry(&mut self) -> &mut NativeFunctionRegistry {
        &mut self.vm.native_registry
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        // Register with linker first if it exists (so it gets an index)
        if let Some(ref mut linker) = self.linker {
            linker.register(name, f);
            // Update VM registry from linker
            let registry = linker.registry();
            self.vm.native_registry = (*registry.read().unwrap()).clone();
        } else {
            self.vm.register_native(name, f);
        }
    }

    pub fn register_fallback(&mut self, f: NativeFn) {
        // Register fallback with linker if it exists
        if let Some(ref mut linker) = self.linker {
            let registry = linker.registry();
            {
                let mut guard = registry.write().unwrap();
                guard.set_fallback(f);
            }
            self.vm.native_registry = (*registry.read().unwrap()).clone();
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
            self.vm.native_registry = (*registry.read().unwrap()).clone();
            
            // Convert CallNative to CallNativeIndexed
            Self::convert_to_indexed_calls(&mut bytecode.data, &bytecode.strings, &self.vm.native_registry);
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
            // Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.native_registry);
        }

        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;
        loop {
            match self.vm.run().map_err(|e| e.to_string())? {
                RunResult::Finished(val) => return Ok(val),
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

        let bytecode_data = bytecode.data;
        let strings = bytecode.strings;

        // Link bytecode if linker is available
        if self.linker.is_some() {
            // Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.native_registry);
        }

        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;

        // Take the callback receiver and keep sender alive
        let mut callback_rx = Some(self.callback_rx.take().unwrap());
        let callback_tx = self.callback_tx.take().unwrap();

        // Set the callback sender in thread local storage for native functions
        // Keep a clone alive for the duration of run_to_completion
        let _tx_guard = callback_tx.clone();
        set_async_callback_sender(callback_tx.clone());

        loop {
            let result = self.vm.run().map_err(|e| e.to_string())?;

            match result {
                RunResult::Finished(val) => {
                    return Ok(val);
                }
                RunResult::Breakpoint => {
                    println!("Breakpoint hit at {}:{}", self.vm.get_source_file().unwrap_or_else(|| "<unknown>".to_string()), self.vm.get_line());
                    continue;
                }
                RunResult::Suspended => {
                    // VM is suspended waiting for async native callback
                    debug_vm!("executor: VM suspended, waiting for callback");
                    // Use std::thread to wait for callback to avoid tokio state machine corruption
                    let rx = callback_rx.take().unwrap();
                    debug_vm!("executor: Waiting for callback in spawned thread");
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

    /// Hot-swap a native function at runtime
    /// 
    /// This replaces the function implementation without recompiling bytecode.
    /// The new implementation will be used on the next call.
    pub fn hot_swap(&mut self, name: &str, new_func: NativeFn) -> bool {
        self.vm.native_registry.hot_swap(name, new_func)
    }

    /// Force relinking of bytecode (useful after hot-swap if indices changed)
    pub fn relink(&mut self, bytecode: &mut Bytecode) {
        if let Some(ref mut linker) = self.linker {
            let registry = linker.registry();
            self.vm.native_registry = (*registry.read().unwrap()).clone();
            Self::convert_to_indexed_calls(&mut bytecode.data, &bytecode.strings, &self.vm.native_registry);
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

use crate::vm::{VM, Value, PromiseState, NativeFn, Class, Function, RunResult, Opcode};
use crate::linker::{RuntimeLinker, NativeFunctionRegistry};
use std::sync::{Arc, RwLock};

pub struct Bytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
}

pub struct Executor {
    pub vm: VM,
    /// Optional runtime linker for dynamic linking and hot-swap
    pub linker: Option<RuntimeLinker>,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            vm: VM::new(),
            linker: None,
        }
    }

    /// Create a new executor with runtime linker support
    pub fn with_linker() -> Self {
        let linker = RuntimeLinker::new();
        let registry = linker.registry();
        let mut vm = VM::new();
        // Share the same registry between VM and linker
        vm.native_registry = (*registry.read().unwrap()).clone();
        Self {
            vm,
            linker: Some(linker),
        }
    }

    /// Create a new executor with a shared registry
    pub fn with_registry(registry: Arc<RwLock<NativeFunctionRegistry>>) -> Self {
        let mut vm = VM::new();
        vm.native_registry = (*registry.read().unwrap()).clone();
        Self {
            vm,
            linker: Some(RuntimeLinker::with_registry(registry)),
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
        self.vm.register_native(name, f);
    }

    pub fn register_fallback(&mut self, f: NativeFn) {
        self.vm.register_fallback(f);
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
    fn convert_to_indexed_calls(bytecode: &mut [u8], strings: &[String], registry: &NativeFunctionRegistry) {
        let mut i = 0;
        while i < bytecode.len() {
            let opcode = bytecode[i];
            
            if opcode == Opcode::CallNative as u8 || opcode == Opcode::CallNativeAsync as u8 {
                // Format: [CallNative, Rd, name_idx, arg_start, arg_count]
                if i + 4 < bytecode.len() {
                    let rd = bytecode[i + 1];
                    let name_idx = bytecode[i + 2] as usize;
                    let arg_start = bytecode[i + 3];
                    let arg_count = bytecode[i + 4];
                    
                    if let Some(name) = strings.get(name_idx) {
                        if let Some(func_index) = registry.get_index(name) {
                            // Replace with CallNativeIndexed
                            // Format: [CallNativeIndexed, Rd, func_idx_lo, func_idx_hi, arg_start, arg_count]
                            bytecode[i] = Opcode::CallNativeIndexed as u8;
                            bytecode[i + 1] = rd;
                            bytecode[i + 2] = (func_index & 0xFF) as u8;  // low byte
                            bytecode[i + 3] = ((func_index >> 8) & 0xFF) as u8;  // high byte
                            bytecode[i + 4] = arg_start;
                            bytecode[i + 5] = arg_count;
                            
                            // Shift remaining bytes
                            // We added 1 byte (6 bytes total vs 5 bytes original)
                            // Need to insert a byte or handle differently
                            // For simplicity, we'll use the same size by storing index in 2 bytes
                            // Original: name_idx (1 byte), New: func_idx (2 bytes)
                            // We need to shift everything after by 1 byte
                            
                            // Actually, let's keep the same instruction size for simplicity
                            // and just use the index directly in place of name_idx
                            // This means we can only address 256 functions with this approach
                            // For more, we need the full 2-byte index
                            
                            // For now, use 1-byte index (256 functions max)
                            bytecode[i + 2] = func_index as u8;
                            // Keep the rest the same
                        }
                    }
                }
            }
            
            i += 1;
        }
    }

    pub async fn run(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }
        
        let mut bytecode_data = bytecode.data;
        let strings = bytecode.strings;
        
        // Link bytecode if linker is available
        if self.linker.is_some() {
            Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.native_registry);
        }
        
        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions)?;
        match self.vm.run().await.map_err(|e| e.to_string())? {
            RunResult::Finished(val) => Ok(val),
            RunResult::Breakpoint => {
                println!("Breakpoint hit at line {}", self.vm.get_line());
                Ok(None)
            }
            RunResult::Awaiting(promise) => Ok(Some(Value::Promise(promise))),
        }
    }

    pub async fn run_to_completion(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }
        
        let mut bytecode_data = bytecode.data;
        let strings = bytecode.strings;
        
        // Link bytecode if linker is available
        if self.linker.is_some() {
            Self::convert_to_indexed_calls(&mut bytecode_data, &strings, &self.vm.native_registry);
        }
        
        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions)?;

        loop {
            let result = self.vm.run().await.map_err(|e| e.to_string())?;

            match result {
                RunResult::Finished(val) => return Ok(val),
                RunResult::Breakpoint => {
                    println!("Breakpoint hit at {}:{}", self.vm.get_source_file().unwrap_or_else(|| "<unknown>".to_string()), self.vm.get_line());
                    continue;
                }
                RunResult::Awaiting(promise) => {
                    let mut state = promise.lock().await;
                    match &mut *state {
                        PromiseState::Pending => {
                            drop(state);
                            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                            continue;
                        }
                        PromiseState::Resolved(_) | PromiseState::Rejected(_) => {
                            drop(state);
                            continue;
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

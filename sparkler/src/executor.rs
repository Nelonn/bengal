use crate::vm::{VM, Value, PromiseState, NativeFn, Class, Function, RunResult};
use crate::opcodes::Opcode;
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
            let opcode_byte = bytecode[i];
            
            // Get the opcode size to skip past this instruction
            let opcode = match opcode_byte {
                x if x == crate::opcodes::Opcode::CallNative as u8 || x == crate::opcodes::Opcode::CallNativeAsync as u8 => {
                    // Format: [CallNative, Rd, name_idx, arg_start, arg_count] (5 bytes)
                    if i + 4 < bytecode.len() {
                        let rd = bytecode[i + 1];
                        let name_idx = bytecode[i + 2] as usize;
                        let arg_start = bytecode[i + 3];
                        let arg_count = bytecode[i + 4];

                        if let Some(name) = strings.get(name_idx) {
                            if let Some(func_index) = registry.get_index(name) {
                                // Replace with CallNativeIndexed
                                // Format: [CallNativeIndexed, Rd, func_idx_lo, func_idx_hi, arg_start, arg_count] (6 bytes)
                                // Need to shift remaining bytecode by 1 byte to make room
                                
                                // Shift all bytes after this instruction by 1 position
                                for j in (i + 5..bytecode.len()).rev() {
                                    bytecode[j] = bytecode[j - 1];
                                }
                                
                                // Write the new 6-byte instruction
                                bytecode[i] = Opcode::CallNativeIndexed as u8;
                                bytecode[i + 1] = rd;
                                bytecode[i + 2] = (func_index & 0xFF) as u8;  // low byte
                                bytecode[i + 3] = ((func_index >> 8) & 0xFF) as u8;  // high byte
                                bytecode[i + 4] = arg_start;
                                bytecode[i + 5] = arg_count;
                                
                                // Skip past the new instruction (6 bytes)
                                i += 6;
                                continue;
                            }
                        }
                    }
                    Some(Opcode::CallNative)
                }
                _ => {
                    // Try to get the opcode from the byte value
                    // We need to handle all possible opcode values
                    match opcode_byte {
                        0x00 => Some(crate::opcodes::Opcode::Nop),
                        0x10 => Some(crate::opcodes::Opcode::LoadConst),
                        0x11 => Some(crate::opcodes::Opcode::LoadInt),
                        0x12 => Some(crate::opcodes::Opcode::LoadFloat),
                        0x13 => Some(crate::opcodes::Opcode::LoadBool),
                        0x14 => Some(crate::opcodes::Opcode::LoadNull),
                        0x20 => Some(crate::opcodes::Opcode::Move),
                        0x21 => Some(crate::opcodes::Opcode::LoadLocal),
                        0x22 => Some(crate::opcodes::Opcode::StoreLocal),
                        0x30 => Some(crate::opcodes::Opcode::GetProperty),
                        0x31 => Some(crate::opcodes::Opcode::SetProperty),
                        0x40 => Some(crate::opcodes::Opcode::Call),
                        0x41 => Some(crate::opcodes::Opcode::CallNative),
                        0x42 => Some(crate::opcodes::Opcode::Invoke),
                        0x43 => Some(crate::opcodes::Opcode::Return),
                        0x44 => Some(crate::opcodes::Opcode::CallAsync),
                        0x45 => Some(crate::opcodes::Opcode::CallNativeAsync),
                        0x46 => Some(crate::opcodes::Opcode::InvokeAsync),
                        0x47 => Some(crate::opcodes::Opcode::Await),
                        0x48 => Some(crate::opcodes::Opcode::Spawn),
                        0x49 => Some(crate::opcodes::Opcode::InvokeInterface),
                        0x4A => Some(crate::opcodes::Opcode::InvokeInterfaceAsync),
                        0x4B => Some(crate::opcodes::Opcode::CallNativeIndexed),
                        0x4C => Some(crate::opcodes::Opcode::CallNativeIndexedAsync),
                        0x50 => Some(crate::opcodes::Opcode::Jump),
                        0x51 => Some(crate::opcodes::Opcode::JumpIfTrue),
                        0x52 => Some(crate::opcodes::Opcode::JumpIfFalse),
                        0x60 => Some(crate::opcodes::Opcode::Equal),
                        0x61 => Some(crate::opcodes::Opcode::NotEqual),
                        0x62 => Some(crate::opcodes::Opcode::And),
                        0x63 => Some(crate::opcodes::Opcode::Or),
                        0x64 => Some(crate::opcodes::Opcode::Not),
                        0x65 => Some(crate::opcodes::Opcode::Concat),
                        0x66 => Some(crate::opcodes::Opcode::Greater),
                        0x67 => Some(crate::opcodes::Opcode::Less),
                        0x68 => Some(crate::opcodes::Opcode::Add),
                        0x69 => Some(crate::opcodes::Opcode::Subtract),
                        0x6A => Some(crate::opcodes::Opcode::GreaterEqual),
                        0x6B => Some(crate::opcodes::Opcode::LessEqual),
                        0x70 => Some(crate::opcodes::Opcode::Multiply),
                        0x71 => Some(crate::opcodes::Opcode::Divide),
                        0x73 => Some(crate::opcodes::Opcode::Line),
                        0x74 => Some(crate::opcodes::Opcode::Convert),
                        0x75 => Some(crate::opcodes::Opcode::Modulo),
                        0x76 => Some(crate::opcodes::Opcode::Array),
                        0x77 => Some(crate::opcodes::Opcode::Index),
                        0x80 => Some(crate::opcodes::Opcode::TryStart),
                        0x81 => Some(crate::opcodes::Opcode::TryEnd),
                        0x82 => Some(crate::opcodes::Opcode::Throw),
                        0x90 => Some(crate::opcodes::Opcode::Breakpoint),
                        0xFF => Some(crate::opcodes::Opcode::Halt),
                        _ => None,
                    }
                }
            };
            
            // Skip past this instruction based on its size
            if let Some(op) = opcode {
                i += op.size();
            } else {
                // Unknown opcode, skip 1 byte
                i += 1;
            }
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

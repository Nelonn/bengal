use crate::vm::{VM, Value, PromiseState, NativeFn, Class, Function, RunResult};
use crate::opcodes::Opcode;
use crate::linker::{RuntimeLinker, NativeFunctionRegistry};
use crate::async_runtime;
use std::sync::{Arc, RwLock};

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
                        0x00 => Some(Opcode::Nop),
                        0x10 => Some(Opcode::LoadConst),
                        0x11 => Some(Opcode::LoadInt),
                        0x12 => Some(Opcode::LoadFloat),
                        0x13 => Some(Opcode::LoadBool),
                        0x14 => Some(Opcode::LoadNull),
                        0x20 => Some(Opcode::Move),
                        0x21 => Some(Opcode::LoadLocal),
                        0x22 => Some(Opcode::StoreLocal),
                        0x30 => Some(Opcode::GetProperty),
                        0x31 => Some(Opcode::SetProperty),
                        0x40 => Some(Opcode::Call),
                        0x41 => Some(Opcode::CallNative),
                        0x42 => Some(Opcode::Invoke),
                        0x43 => Some(Opcode::Return),
                        0x44 => Some(Opcode::CallAsync),
                        0x45 => Some(Opcode::CallNativeAsync),
                        0x46 => Some(Opcode::InvokeAsync),
                        0x47 => Some(Opcode::Await),
                        0x48 => Some(Opcode::Spawn),
                        0x49 => Some(Opcode::InvokeInterface),
                        0x4A => Some(Opcode::InvokeInterfaceAsync),
                        0x4B => Some(Opcode::CallNativeIndexed),
                        0x4C => Some(Opcode::CallNativeIndexedAsync),
                        0x50 => Some(Opcode::Jump),
                        0x51 => Some(Opcode::JumpIfTrue),
                        0x52 => Some(Opcode::JumpIfFalse),
                        0x60 => Some(Opcode::Equal),
                        0x61 => Some(Opcode::NotEqual),
                        0x62 => Some(Opcode::And),
                        0x63 => Some(Opcode::Or),
                        0x64 => Some(Opcode::Not),
                        0x65 => Some(Opcode::Concat),
                        0x66 => Some(Opcode::Greater),
                        0x67 => Some(Opcode::Less),
                        0x68 => Some(Opcode::Add),
                        0x69 => Some(Opcode::Subtract),
                        0x6A => Some(Opcode::GreaterEqual),
                        0x6B => Some(Opcode::LessEqual),
                        0x70 => Some(Opcode::Multiply),
                        0x71 => Some(Opcode::Divide),
                        0x73 => Some(Opcode::Line),
                        0x74 => Some(Opcode::Convert),
                        0x75 => Some(Opcode::Modulo),
                        0x76 => Some(Opcode::Array),
                        0x77 => Some(Opcode::Index),
                        0x78 => Some(Opcode::BitAnd),
                        0x79 => Some(Opcode::BitOr),
                        0x7A => Some(Opcode::BitXor),
                        0x7B => Some(Opcode::BitNot),
                        0x7C => Some(Opcode::ShiftLeft),
                        0x7D => Some(Opcode::ShiftRight),
                        0x80 => Some(Opcode::TryStart),
                        0x81 => Some(Opcode::TryEnd),
                        0x82 => Some(Opcode::Throw),
                        0x90 => Some(Opcode::Breakpoint),
                        0xFF => Some(Opcode::Halt),
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
        
        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;
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
        
        self.vm.load(&bytecode_data, strings, bytecode.classes, bytecode.functions, bytecode.vtables)?;

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
                            async_runtime::sleep(std::time::Duration::from_millis(10)).await;
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

use crate::vm::{VM, Value, PromiseState, NativeFn, Class, Function, RunResult};

pub struct Bytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub classes: Vec<Class>,
    pub functions: Vec<Function>,
}

pub struct Executor {
    pub vm: VM,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            vm: VM::new(),
        }
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        self.vm.register_native(name, f);
    }

    pub fn register_fallback(&mut self, f: NativeFn) {
        self.vm.register_fallback(f);
    }

    pub async fn run(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }
        self.vm.load(&bytecode.data, bytecode.strings, bytecode.classes, bytecode.functions)?;
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
        self.vm.load(&bytecode.data, bytecode.strings, bytecode.classes, bytecode.functions)?;

        loop {
            let result = self.vm.run().await.map_err(|e| e.to_string())?;

            match result {
                RunResult::Finished(val) => return Ok(val),
                RunResult::Breakpoint => {
                    println!("Breakpoint hit at {}:{}", self.vm.get_source_file().unwrap_or_else(|| "<unknown>".to_string()), self.vm.get_line());
                    // In run_to_completion, we just continue after a breakpoint for now
                    // In a real debugger, we would wait for user input
                    continue;
                }
                RunResult::Awaiting(promise) => {
                    // Wait for the promise to resolve
                    let mut state = promise.lock().await;
                    match &mut *state {
                        PromiseState::Pending => {
                            // Wait for it to resolve
                            drop(state);
                            // We don't have a good way to wait for the promise from here without blocking
                            // In a real VM, this would be handled by the event loop
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
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

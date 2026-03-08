use crate::vm::{VM, Value, PromiseState, NativeFn, Class};

pub struct Bytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub classes: Vec<Class>,
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

    pub async fn run(&mut self, bytecode: Bytecode) -> Result<Option<Value>, String> {
        self.vm.load(&bytecode.data, bytecode.strings, bytecode.classes)?;
        self.vm.run().await
    }

    pub async fn run_to_completion(&mut self, bytecode: Bytecode) -> Result<Option<Value>, String> {
        self.vm.load(&bytecode.data, bytecode.strings, bytecode.classes)?;
        
        loop {
            let result = self.vm.run().await?;
            
            match result {
                Some(Value::Promise(promise)) => {
                    let state = promise.lock().await;
                    if matches!(*state, PromiseState::Resolved(_) | PromiseState::Rejected(_)) {
                        drop(state);
                        continue;
                    }
                    drop(state);
                    return Ok(Some(Value::Promise(promise)));
                }
                _ => return Ok(result),
            }
        }
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}

use crate::vm::{VM, Value, PromiseState, NativeFn, Class, Function};

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
        self.vm.run().await.map_err(|e| e.to_string())
    }

    pub async fn run_to_completion(&mut self, bytecode: Bytecode, source_file: Option<&str>) -> Result<Option<Value>, String> {
        if let Some(file) = source_file {
            self.vm.set_source_file(file);
        }
        self.vm.load(&bytecode.data, bytecode.strings, bytecode.classes, bytecode.functions)?;

        loop {
            let result = self.vm.run().await.map_err(|e| e.to_string())?;

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

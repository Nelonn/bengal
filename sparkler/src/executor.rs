use crate::vm::{VM, Value, PromiseState};

pub struct Bytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
}

pub struct Executor {
    vm: VM,
}

impl Executor {
    pub fn new() -> Self {
        Self {
            vm: VM::new(),
        }
    }

    pub async fn run(&mut self, bytecode: Bytecode) -> Result<Option<Value>, String> {
        self.vm.load(&bytecode.data, bytecode.strings)?;
        self.vm.run().await
    }

    pub async fn run_to_completion(&mut self, bytecode: Bytecode) -> Result<Option<Value>, String> {
        self.vm.load(&bytecode.data, bytecode.strings)?;
        
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

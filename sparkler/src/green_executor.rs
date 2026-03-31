/// Green Thread Executor
///
/// Executes bytecode using green threads with cooperative multitasking.
/// Supports the `spawn` keyword for creating new threads.

use crate::vm::{VM, Value, NativeFn, NativeResult};
use crate::scheduler::{Scheduler, ThreadId};
use crate::linker::RuntimeLinker;
use crate::executor::Bytecode;
use std::sync::mpsc::{channel, Sender, Receiver};
use std::cell::RefCell;

thread_local! {
    static SCHEDULER_PTR: RefCell<Option<*mut Scheduler>> = RefCell::new(None);
    static BYTECODE_STORE: RefCell<Option<Bytecode>> = RefCell::new(None);
}

/// Get the current scheduler (for use in native functions)
fn with_scheduler<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&mut Scheduler) -> R,
{
    SCHEDULER_PTR.with(|cell| {
        if let Some(ptr) = cell.borrow().as_ref() {
            let scheduler = unsafe { &mut **ptr };
            Some(f(scheduler))
        } else {
            None
        }
    })
}

/// Get a copy of the bytecode for spawning new threads
fn get_bytecode() -> Option<Bytecode> {
    BYTECODE_STORE.with(|cell| cell.borrow().as_ref().cloned())
}

/// Native function for spawning a new green thread
/// Usage: __spawn("function_name", arg1, arg2, ...)
fn native_spawn(args: &mut Vec<Value>) -> NativeResult {
    if args.is_empty() {
        return NativeResult::Ready(Value::Null);
    }

    // First argument should be the function name (string)
    let func_name = match &args[0] {
        Value::String(s) => s.clone(),
        _ => return NativeResult::Ready(Value::Null),
    };

    // Remaining arguments are passed to the function
    let func_args: Vec<Value> = args[1..].to_vec();

    // Get the bytecode and create a new VM
    if let Some(bytecode) = get_bytecode() {
        let mut vm = VM::new();
        if vm.load(
            &bytecode.data,
            bytecode.strings,
            bytecode.classes,
            bytecode.functions,
            bytecode.vtables,
        ).is_ok() {
            // Call the function in the new VM
            if vm.call_function(&func_name, func_args).is_ok() {
                // Spawn the new thread
                with_scheduler(|scheduler| {
                    scheduler.spawn(vm);
                });
            }
        }
    }

    NativeResult::Ready(Value::Null)
}

/// Green thread executor with scheduler support
pub struct GreenThreadExecutor {
    scheduler: Scheduler,
    linker: Option<RuntimeLinker>,
    callback_tx: Option<Sender<Result<Value, Value>>>,
    callback_rx: Option<Receiver<Result<Value, Value>>>,
    /// Bytecode shared across all threads
    bytecode: Option<Bytecode>,
}

impl GreenThreadExecutor {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        Self {
            scheduler: Scheduler::new(),
            linker: None,
            callback_tx: Some(tx),
            callback_rx: Some(rx),
            bytecode: None,
        }
    }

    /// Create executor with runtime linker support
    pub fn with_linker() -> Self {
        let linker = RuntimeLinker::new();
        let (tx, rx) = channel();
        Self {
            scheduler: Scheduler::new(),
            linker: Some(linker),
            callback_tx: Some(tx),
            callback_rx: Some(rx),
            bytecode: None,
        }
    }

    /// Register a native function
    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        if let Some(ref mut linker) = self.linker {
            linker.register(name, f);
        }
    }

    /// Load bytecode and prepare for execution
    pub fn load(&mut self, bytecode: Bytecode) -> Result<(), String> {
        self.bytecode = Some(bytecode);
        Ok(())
    }

    /// Spawn the main thread and run until completion
    pub fn run(&mut self, main_function: &str) -> Result<Option<Value>, String> {
        let bytecode = self.bytecode.take().ok_or("No bytecode loaded")?;

        // Store bytecode in thread-local for spawn to access
        BYTECODE_STORE.with(|cell| {
            *cell.borrow_mut() = Some(bytecode.clone());
        });

        // Create the main VM
        let mut vm = VM::new();
        vm.load(
            &bytecode.data,
            bytecode.strings.clone(),
            bytecode.classes.clone(),
            bytecode.functions.clone(),
            bytecode.vtables.clone(),
        )?;

        // Register the __spawn native function
        vm.register_native("__spawn", native_spawn);

        // Call the main function
        vm.call_function(main_function, vec![])?;

        // Spawn the main thread
        self.scheduler.spawn(vm);

        // Set up thread-local scheduler pointer
        let scheduler_ptr = &mut self.scheduler as *mut Scheduler;
        SCHEDULER_PTR.with(|cell| {
            *cell.borrow_mut() = Some(scheduler_ptr);
        });

        // Run the scheduler
        let (result, _has_blocked) = self.scheduler.run();

        // Clear thread-local
        SCHEDULER_PTR.with(|cell| {
            *cell.borrow_mut() = None;
        });
        BYTECODE_STORE.with(|cell| {
            *cell.borrow_mut() = None;
        });

        Ok(result)
    }

    /// Spawn a new thread from an existing VM (used internally)
    pub fn spawn_vm(&mut self, vm: VM) -> ThreadId {
        self.scheduler.spawn(vm)
    }

    /// Get the number of active threads
    pub fn active_thread_count(&self) -> usize {
        self.scheduler.active_thread_count()
    }

    /// Get the number of ready threads
    pub fn ready_thread_count(&self) -> usize {
        self.scheduler.ready_thread_count()
    }

    /// Get the number of blocked threads
    pub fn blocked_thread_count(&self) -> usize {
        self.scheduler.blocked_thread_count()
    }
}

impl Default for GreenThreadExecutor {
    fn default() -> Self {
        Self::new()
    }
}

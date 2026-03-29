pub mod vm;
pub mod executor;
pub mod linker;
pub mod opcodes;
pub mod async_runtime;

pub use vm::{VM, Value, NativeFn, NativeFnAsync, NativeFallbackFn, NativeResult, Exception, StackFrame, NativeFunctionBuilder, NativeModule, NativeClass, NativeClassBuilder, Function, Class, Method, VTable, PromiseState, RunResult, ExecutionResult, set_async_callback_sender, get_async_callback_sender};
pub use executor::{Executor, Bytecode};
pub use linker::{NativeFunctionRegistry, RuntimeLinker, NativeFunctionEntry};
pub use opcodes::Opcode;
pub use async_runtime::Mutex;

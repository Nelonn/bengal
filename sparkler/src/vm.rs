use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::ser::SerializeMap;
use serde::de::{MapAccess, Visitor};
use std::fmt;
use std::any::Any;
use crate::linker::NativeFunctionRegistry;
use crate::opcodes::Opcode;
use crate::async_runtime::Mutex as AsyncMutex;

#[macro_export]
macro_rules! debug_vm {
    ($($arg:tt)*) => {
        if cfg!(feature = "debug_log") {
            println!($($arg)*);
        }
    };
}

thread_local! {
    pub static ASYNC_CALLBACK_SENDER: std::cell::RefCell<Option<std::sync::mpsc::Sender<Result<Value, Value>>>> = std::cell::RefCell::new(None);
}

pub fn set_async_callback_sender(tx: std::sync::mpsc::Sender<Result<Value, Value>>) {
    ASYNC_CALLBACK_SENDER.with(|s| {
        *s.borrow_mut() = Some(tx);
    });
}

pub fn get_async_callback_sender() -> Option<std::sync::mpsc::Sender<Result<Value, Value>>> {
    let result = ASYNC_CALLBACK_SENDER.with(|s| s.borrow().clone());
    result
}

/// Extract base class name from generic type syntax (e.g., "Array<int>" -> "Array")
/// or from constructor names (e.g., "SomeObject.constructor(str)" -> "SomeObject")
fn extract_base_class_name(name: &str) -> &str {
    // Handle generic types like Array<T>
    if let Some(angle_pos) = name.find('<') {
        return &name[..angle_pos];
    }

    // Handle constructor names like SomeObject.constructor(str)
    if let Some(constructor_pos) = name.find(".constructor(") {
        return &name[..constructor_pos];
    }

    name
}

pub type Bytecode = Vec<u8>;

/// Represents a single frame in the call stack
#[derive(Clone, Debug)]
pub struct StackFrame {
    pub function_name: String,
    pub source_file: Option<String>,
    pub line_number: Option<usize>,
}

impl fmt::Display for StackFrame {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "  at {}", self.function_name)?;
        if let Some(file) = &self.source_file {
            write!(f, " ({})", file)?;
            if let Some(line) = self.line_number {
                write!(f, ":{}", line)?;
            }
        }
        Ok(())
    }
}

/// Exception with stack trace information
#[derive(Clone, Debug)]
pub struct Exception {
    pub message: String,
    pub stack_trace: Vec<StackFrame>,
}

impl fmt::Display for Exception {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Exception: {}", self.message)?;
        if !self.stack_trace.is_empty() {
            writeln!(f, "Stack trace:")?;
            for frame in self.stack_trace.iter().rev() {
                writeln!(f, "{}", frame)?;
            }
        }
        Ok(())
    }
}

impl Exception {
    pub fn new(message: String, stack_trace: Vec<StackFrame>) -> Self {
        Self { message, stack_trace }
    }

    pub fn with_message(message: &str) -> Self {
        Self {
            message: message.to_string(),
            stack_trace: Vec::new(),
        }
    }
}

/// All supported numeric types for VM and FFI
#[derive(Clone, Copy, Debug)]
pub enum IntValue {
    U8(u8),
    I8(i8),
    U16(u16),
    I16(i16),
    U32(u32),
    I32(i32),
    U64(u64),
    I64(i64),
}

#[derive(Clone, Copy, Debug)]
pub enum FloatValue {
    F32(f32),
    F64(f64),
}

#[derive(Clone)]
pub enum Value {
    String(String),
    Int8(i8),
    Int16(i16),
    Int32(i32),
    Int64(i64),
    UInt8(u8),
    UInt16(u16),
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    Null,
    Instance(Arc<Mutex<Instance>>),
    Array(Arc<Mutex<Vec<Value>>>),
    Exception(Exception),
    Promise(Arc<AsyncMutex<PromiseState>>),
}

impl fmt::Debug for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::String(s) => write!(f, "String({})", s),
            Value::Int8(n) => write!(f, "Int8({})", n),
            Value::Int16(n) => write!(f, "Int16({})", n),
            Value::Int32(n) => write!(f, "Int32({})", n),
            Value::Int64(n) => write!(f, "Int64({})", n),
            Value::UInt8(n) => write!(f, "UInt8({})", n),
            Value::UInt16(n) => write!(f, "UInt16({})", n),
            Value::UInt32(n) => write!(f, "UInt32({})", n),
            Value::UInt64(n) => write!(f, "UInt64({})", n),
            Value::Float32(n) => write!(f, "Float32({})", n),
            Value::Float64(n) => write!(f, "Float64({})", n),
            Value::Bool(b) => write!(f, "Bool({})", b),
            Value::Null => write!(f, "Null"),
            Value::Instance(_) => write!(f, "Instance(...)"),
            Value::Array(_) => write!(f, "Array(...)"),
            Value::Exception(e) => write!(f, "Exception({})", e.message),
            Value::Promise(_) => write!(f, "Promise(...)"),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Instance(a), Value::Instance(b)) => Arc::ptr_eq(a, b),
            (Value::Array(a), Value::Array(b)) => Arc::ptr_eq(a, b),
            (Value::Exception(a), Value::Exception(b)) => a.message == b.message,
            (Value::Promise(a), Value::Promise(b)) => Arc::ptr_eq(a, b),
            (Value::Int64(a), Value::Int64(b)) => a == b,
            (Value::Int64(a), Value::Int8(b)) => *a == *b as i64,
            (Value::Int64(a), Value::Int16(b)) => *a == *b as i64,
            (Value::Int64(a), Value::Int32(b)) => *a == *b as i64,
            (Value::Int64(a), Value::UInt8(b)) => *a == *b as i64,
            (Value::Int64(a), Value::UInt16(b)) => *a == *b as i64,
            (Value::Int64(a), Value::UInt32(b)) => *a == *b as i64,
            (Value::Int64(a), Value::UInt64(b)) => *a == *b as i64,
            (Value::Int8(a), Value::Int64(b)) => *a as i64 == *b,
            (Value::Int8(a), Value::Int8(b)) => a == b,
            (Value::Int16(a), Value::Int16(b)) => a == b,
            (Value::Int32(a), Value::Int32(b)) => a == b,
            (Value::UInt8(a), Value::UInt8(b)) => a == b,
            (Value::UInt16(a), Value::UInt16(b)) => a == b,
            (Value::UInt32(a), Value::UInt32(b)) => a == b,
            (Value::UInt64(a), Value::UInt64(b)) => a == b,
            (Value::Float64(a), Value::Float64(b)) => a == b,
            (Value::Float64(a), Value::Float32(b)) => *a == *b as f64,
            (Value::Float32(a), Value::Float64(b)) => *a as f64 == *b,
            (Value::Float32(a), Value::Float32(b)) => a == b,
            (Value::Int64(a), Value::Float64(b)) => (*a as f64) == *b,
            (Value::Float64(a), Value::Int64(b)) => *a == (*b as f64),
            _ => false,
        }
    }
}

impl Value {
    pub fn to_i64(&self) -> Option<i64> {
        match self {
            Value::Int64(n) => Some(*n),
            Value::Int8(n) => Some(*n as i64),
            Value::Int16(n) => Some(*n as i64),
            Value::Int32(n) => Some(*n as i64),
            Value::UInt8(n) => Some(*n as i64),
            Value::UInt16(n) => Some(*n as i64),
            Value::UInt32(n) => Some(*n as i64),
            Value::UInt64(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Float64(n) => Some(*n),
            Value::Float32(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn is_arithmetic_int(&self) -> bool {
        matches!(self, Value::Int64(_) | Value::Int32(_) | Value::UInt32(_) | Value::UInt64(_))
    }

    pub fn is_arithmetic_float(&self) -> bool {
        matches!(self, Value::Float64(_) | Value::Float32(_))
    }

    pub fn to_arithmetic_int(&self) -> Option<i64> {
        match self {
            Value::Int64(n) => Some(*n),
            Value::Int32(n) => Some(*n as i64),
            Value::UInt32(n) => Some(*n as i64),
            Value::UInt64(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn to_int(&self) -> Option<i64> {
        match self {
            Value::Int64(n) => Some(*n),
            Value::Int8(n) => Some(*n as i64),
            Value::Int16(n) => Some(*n as i64),
            Value::Int32(n) => Some(*n as i64),
            Value::UInt8(n) => Some(*n as i64),
            Value::UInt16(n) => Some(*n as i64),
            Value::UInt32(n) => Some(*n as i64),
            Value::UInt64(n) => Some(*n as i64),
            Value::Float64(n) => Some(*n as i64),
            Value::Float32(n) => Some(*n as i64),
            _ => None,
        }
    }

    pub fn to_float(&self) -> Option<f64> {
        match self {
            Value::Int64(n) => Some(*n as f64),
            Value::Int8(n) => Some(*n as f64),
            Value::Int16(n) => Some(*n as f64),
            Value::Int32(n) => Some(*n as f64),
            Value::UInt8(n) => Some(*n as f64),
            Value::UInt16(n) => Some(*n as f64),
            Value::UInt32(n) => Some(*n as f64),
            Value::UInt64(n) => Some(*n as f64),
            Value::Float64(n) => Some(*n),
            Value::Float32(n) => Some(*n as f64),
            _ => None,
        }
    }

    pub fn to_u8(&self) -> Option<u8> {
        self.to_int().map(|n| n as u8)
    }

    pub fn to_i8(&self) -> Option<i8> {
        self.to_int().map(|n| n as i8)
    }

    pub fn to_u16(&self) -> Option<u16> {
        self.to_int().map(|n| n as u16)
    }

    pub fn to_i16(&self) -> Option<i16> {
        self.to_int().map(|n| n as i16)
    }

    pub fn to_u32(&self) -> Option<u32> {
        self.to_int().map(|n| n as u32)
    }

    pub fn to_i32(&self) -> Option<i32> {
        self.to_int().map(|n| n as i32)
    }

    pub fn to_u64(&self) -> Option<u64> {
        self.to_int().map(|n| n as u64)
    }

    pub fn to_f32(&self) -> Option<f32> {
        self.to_float().map(|n| n as f32)
    }

    pub fn is_truthy(&self) -> bool {
        match self {
            Value::Bool(false) | Value::Null => false,
            Value::Int64(0) => false,
            Value::Int8(0) => false,
            Value::Int16(0) => false,
            Value::Int32(0) => false,
            Value::UInt8(0) => false,
            Value::UInt16(0) => false,
            Value::UInt32(0) => false,
            Value::UInt64(0) => false,
            Value::Float64(0.0) => false,
            Value::Float32(0.0) => false,
            _ => true,
        }
    }

    pub fn to_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Int8(n) => n.to_string(),
            Value::Int16(n) => n.to_string(),
            Value::Int32(n) => n.to_string(),
            Value::Int64(n) => n.to_string(),
            Value::UInt8(n) => n.to_string(),
            Value::UInt16(n) => n.to_string(),
            Value::UInt32(n) => n.to_string(),
            Value::UInt64(n) => n.to_string(),
            Value::Float32(n) => n.to_string(),
            Value::Float64(n) => n.to_string(),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".to_string(),
            Value::Instance(inst) => {
                let inst = inst.lock().unwrap();
                let mut fields_str = Vec::new();
                for (key, value) in &inst.fields {
                    let value_str = match value {
                        Value::String(s) => format!("\"{}\"", s),
                        Value::Int8(n) => n.to_string(),
                        Value::Int16(n) => n.to_string(),
                        Value::Int32(n) => n.to_string(),
                        Value::Int64(n) => n.to_string(),
                        Value::UInt8(n) => n.to_string(),
                        Value::UInt16(n) => n.to_string(),
                        Value::UInt32(n) => n.to_string(),
                        Value::UInt64(n) => n.to_string(),
                        Value::Float32(n) => n.to_string(),
                        Value::Float64(n) => n.to_string(),
                        Value::Bool(b) => b.to_string(),
                        Value::Null => "null".to_string(),
                        Value::Instance(_) => "[instance]".to_string(),
                        Value::Array(_) => "[array]".to_string(),
                        Value::Exception(e) => format!("[exception: {}]", e.message),
                        Value::Promise(_) => "[promise]".to_string(),
                    };
                    fields_str.push(format!("\"{}\": {}", key, value_str));
                }
                format!("{{ {} }}", fields_str.join(", "))
            }
            Value::Array(arr) => {
                let arr = arr.lock().unwrap();
                let elements_str: Vec<String> = arr.iter().map(|v| v.to_string()).collect();
                format!("[{}]", elements_str.join(", "))
            }
            Value::Exception(e) => e.to_string(),
            Value::Promise(_) => "[promise]".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Instance {
    pub class: String,
    pub fields: HashMap<String, Value>,
    pub private_fields: HashSet<String>,
    pub native_data: Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>,
}

#[derive(Clone, Debug)]
pub struct Class {
    pub name: String,
    pub fields: HashMap<String, Value>,
    pub private_fields: HashSet<String>,
    pub methods: HashMap<String, Method>,
    pub native_methods: HashMap<String, NativeFn>,
    pub native_create: Option<NativeFn>,
    pub native_destroy: Option<NativeFn>,
    pub is_native: bool,
    pub parent_interfaces: Vec<String>,
    pub vtable: Vec<String>,  // Ordered list of virtual method names
    pub is_interface: bool,
}

#[derive(Clone, Debug)]
pub struct Method {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub register_count: u8,
}

#[derive(Clone, Debug)]
pub struct Function {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub param_count: u8,
    pub register_count: u8,
    pub source_file: Option<String>,
}

/// Result from a native function call
#[derive(Debug)]
pub enum NativeResult {
    /// Function completed immediately with a value
    Ready(Value),
    /// Function is pending, will callback with result later
    Pending,
    /// Function is pending with a wait identifier for targeted wakeup
    PendingWithWaitId(String),
}

impl From<Value> for NativeResult {
    fn from(val: Value) -> Self {
        NativeResult::Ready(val)
    }
}

impl From<Result<Value, Value>> for NativeResult {
    fn from(result: Result<Value, Value>) -> Self {
        match result {
            Ok(val) => NativeResult::Ready(val),
            Err(val) => NativeResult::Ready(val),
        }
    }
}

/// Async callback for native functions
pub type AsyncCallback = Box<dyn FnOnce(Result<Value, Value>) + Send + 'static>;

pub type NativeFn = fn(&mut Vec<Value>) -> NativeResult;
pub type NativeFnAsync = fn(&mut Vec<Value>, AsyncCallback) -> NativeResult;
pub type NativeFallbackFn = fn(&str, &mut Vec<Value>) -> NativeResult;

pub struct NativeFunctionBuilder {
    name: String,
    func: NativeFn,
    param_count: Option<usize>,
    return_type: Option<String>,
}

impl NativeFunctionBuilder {
    pub fn new(name: &str, func: NativeFn) -> Self {
        Self {
            name: name.to_string(),
            func,
            param_count: None,
            return_type: None,
        }
    }

    pub fn params(mut self, count: usize) -> Self {
        self.param_count = Some(count);
        self
    }

    pub fn returns(mut self, type_name: &str) -> Self {
        self.return_type = Some(type_name.to_string());
        self
    }

    pub fn register(self, vm: &mut VM) {
        vm.register_native(&self.name, self.func);
    }
}

/// Builder for native class registration with fluent API
pub struct NativeClass {
    class_name: String,
    methods: Vec<(String, NativeFn)>,
    native_create: Option<NativeFn>,
    native_destroy: Option<NativeFn>,
}

impl NativeClass {
    pub fn new(class_name: &str) -> Self {
        Self {
            class_name: class_name.to_string(),
            methods: Vec::new(),
            native_create: None,
            native_destroy: None,
        }
    }

    /// Add a native method to this class
    pub fn method(mut self, method_name: &str, func: NativeFn) -> Self {
        self.methods.push((method_name.to_string(), func));
        self
    }

    /// Set the native constructor callback
    pub fn native_create(mut self, func: NativeFn) -> Self {
        self.native_create = Some(func);
        self
    }

    /// Set the native destructor callback
    pub fn native_destroy(mut self, func: NativeFn) -> Self {
        self.native_destroy = Some(func);
        self
    }

    /// Get the class name
    pub fn class_name(&self) -> &str {
        &self.class_name
    }

    /// Register this class with the VM
    pub fn register(self, vm: &mut VM) {
        let class_name = self.class_name.clone();

        // Register native_create if provided
        if let Some(func) = self.native_create {
            vm.register_class_native_create(&class_name, func);
        }

        // Register native_destroy if provided
        if let Some(func) = self.native_destroy {
            vm.register_class_native_destroy(&class_name, func);
        }

        // Register all methods under the full class name
        for (method_name, func) in &self.methods {
            vm.register_native_method(&class_name, method_name, *func);
        }
        
        // Also register methods under the simple class name (last component)
        // This allows bytecode that uses simple names to find the native methods
        if let Some(simple_name) = class_name.split('.').last() {
            if simple_name != class_name {
                for (method_name, func) in &self.methods {
                    vm.register_native_method(simple_name, method_name, *func);
                }
            }
        }
    }
}

pub struct NativeModule {
    name: String,
    functions: Vec<(String, NativeFn)>,
    classes: Vec<NativeClass>,
}

impl NativeModule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: Vec::new(),
            classes: Vec::new(),
        }
    }

    pub fn function(mut self, name: &str, func: NativeFn) -> Self {
        self.functions.push((name.to_string(), func));
        self
    }

    /// Start defining a native class with fluent API
    ///
    /// # Example
    /// ```ignore
    /// NativeModule::new("std.sys")
    ///     .function("env", sys::native_sys_env)
    ///     .class("Process")
    ///         .native_create(sys::native_process_native_create)
    ///         .native_destroy(sys::native_process_native_destroy)
    ///         .method("start", sys::native_process_start)
    ///         .method("wait", sys::native_process_wait)
    ///         .register_class()
    ///     .register(vm);
    /// ```
    pub fn class(self, class_name: &str) -> NativeClassBuilder {
        // Class names are not prefixed with module name - they are global
        NativeClassBuilder::new(class_name.to_string(), self)
    }

    /// Register a pre-built NativeClass
    pub fn register_class(mut self, class: NativeClass) -> Self {
        self.classes.push(class);
        self
    }

    pub fn register(self, vm: &mut VM) {
        for (name, func) in self.functions {
            let full_name = if self.name.is_empty() {
                name
            } else {
                format!("{}.{}", self.name, name)
            };
            vm.register_native(&full_name, func);
        }
        for class in self.classes {
            class.register(vm);
        }
    }
}

/// Builder for creating a NativeClass within a NativeModule context
pub struct NativeClassBuilder {
    class_name: String,
    methods: Vec<(String, NativeFn)>,
    native_create: Option<NativeFn>,
    native_destroy: Option<NativeFn>,
    module: Option<NativeModule>,
}

impl NativeClassBuilder {
    fn new(class_name: String, module: NativeModule) -> Self {
        Self {
            class_name,
            methods: Vec::new(),
            native_create: None,
            native_destroy: None,
            module: Some(module),
        }
    }

    /// Add a native method to this class
    pub fn method(mut self, method_name: &str, func: NativeFn) -> Self {
        self.methods.push((method_name.to_string(), func));
        self
    }

    /// Set the native constructor callback
    pub fn native_create(mut self, func: NativeFn) -> Self {
        self.native_create = Some(func);
        self
    }

    /// Set the native destructor callback
    pub fn native_destroy(mut self, func: NativeFn) -> Self {
        self.native_destroy = Some(func);
        self
    }

    /// Finish building the class and return to the module builder
    pub fn register_class(self) -> NativeModule {
        let class = NativeClass {
            class_name: self.class_name,
            methods: self.methods,
            native_create: self.native_create,
            native_destroy: self.native_destroy,
        };
        
        let mut module = self.module.unwrap();
        module.classes.push(class);
        module
    }
}

/// A call frame for registry-based execution
/// 
/// Registry-based VMs use a fixed register file per frame.
/// Registers are organized as:
/// - R0: Return value register
/// - R1..Rn: Parameter registers (for callee) / Argument registers (for caller)
/// - R(n+1)..R(max): Local registers
#[derive(Clone, Debug)]
pub struct CallFrame {
    /// Program counter - offset into bytecode
    pub pc: usize,
    /// Frame pointer - base index of this frame's registers in the register file
    pub frame_base: usize,
    /// Number of parameters this function expects
    pub param_count: u8,
    /// Total number of registers used by this frame
    pub register_count: u8,
    /// Function name for debugging and stack traces
    pub function_name: String,
    /// Source file for this frame
    pub source_file: Option<String>,
    /// Current line number
    pub line_number: usize,
    /// Whether this frame is for a native function call
    pub is_native: bool,
    /// Bytecode for this frame
    pub bytecode: Arc<Vec<u8>>,
    /// Strings for this frame
    pub strings: Arc<Vec<String>>,
    /// Functions for this frame
    pub functions: Arc<HashMap<String, Function>>,
    /// Classes for this frame
    pub classes: Arc<HashMap<String, Class>>,
    /// Return register in the caller's frame (None for module-level)
    pub return_reg: Option<u8>,
}

impl CallFrame {
    pub fn new(
        pc: usize,
        frame_base: usize,
        param_count: u8,
        register_count: u8,
        function_name: String,
        source_file: Option<String>,
        bytecode: Arc<Vec<u8>>,
        strings: Arc<Vec<String>>,
        functions: Arc<HashMap<String, Function>>,
        classes: Arc<HashMap<String, Class>>,
        return_reg: Option<u8>,
    ) -> Self {
        Self {
            pc,
            frame_base,
            param_count,
            register_count,
            function_name,
            source_file,
            line_number: 1,
            is_native: false,
            bytecode,
            strings,
            functions,
            classes,
            return_reg,
        }
    }

    pub fn native(
        frame_base: usize,
        param_count: u8,
        function_name: String,
        bytecode: Arc<Vec<u8>>,
        strings: Arc<Vec<String>>,
        functions: Arc<HashMap<String, Function>>,
        classes: Arc<HashMap<String, Class>>,
        return_reg: Option<u8>,
    ) -> Self {
        Self {
            pc: 0,
            frame_base,
            param_count,
            register_count: param_count + 1,
            function_name,
            source_file: None,
            line_number: 0,
            is_native: true,
            bytecode,
            strings,
            functions,
            classes,
            return_reg,
        }
    }
}

/// Execution context - contains runtime state that can be saved/restored when suspended
pub struct Context {
    /// The register file - fixed size array of values
    pub registers: Vec<Value>,
    /// Local variables (for module-level code)
    pub locals: HashMap<String, Value>,
    /// Call stack - frames for active function calls
    pub call_stack: Vec<CallFrame>,
    /// Exception handlers
    pub exception_handlers: Vec<ExceptionHandler>,
    /// Pending native result register (for async native callbacks)
    pub pending_native_result: Option<u8>,
    /// Callback for pending native operation
    pub pending_native_callback: Option<Box<dyn FnOnce(Result<Value, Value>) + Send>>,
    /// Wait identifier for targeted wakeup (used with PendingWithWaitId)
    pub pending_wait_id: Option<String>,
}

impl Clone for Context {
    fn clone(&self) -> Self {
        Self {
            registers: self.registers.clone(),
            locals: self.locals.clone(),
            call_stack: self.call_stack.clone(),
            exception_handlers: self.exception_handlers.clone(),
            pending_native_result: self.pending_native_result,
            pending_native_callback: None, // Cannot clone the callback
            pending_wait_id: self.pending_wait_id.clone(),
        }
    }
}

impl Context {
    pub fn new() -> Self {
        Self {
            registers: vec![Value::Null; 256],
            locals: HashMap::new(),
            call_stack: Vec::new(),
            exception_handlers: Vec::new(),
            pending_native_result: None,
            pending_native_callback: None,
            pending_wait_id: None,
        }
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

/// Loaded program data - shared/immutable bytecode and definitions
#[derive(Clone)]
pub struct Program {
    /// The current bytecode being executed
    pub bytecode: Arc<Vec<u8>>,
    /// String constant pool
    pub strings: Arc<Vec<String>>,
    /// Class definitions
    pub classes: Arc<HashMap<String, Class>>,
    /// Function definitions
    pub functions: Arc<HashMap<String, Function>>,
    /// Native function registry with indexed lookup (optimized)
    pub native_registry: NativeFunctionRegistry,
    /// Fallback native handler
    pub fallback_native: Option<crate::linker::NativeFnType>,
    /// Vtables for interface method dispatch (indexed by vtable_idx)
    pub vtables: Vec<VTable>,
    /// Green thread context (for spawn support)
    pub green_thread_ctx: Option<std::rc::Rc<crate::executor::GreenThreadContext>>,
}

impl Program {
    pub fn new() -> Self {
        Self {
            bytecode: Arc::new(Vec::new()),
            strings: Arc::new(Vec::new()),
            classes: Arc::new(HashMap::new()),
            functions: Arc::new(HashMap::new()),
            native_registry: NativeFunctionRegistry::new(),
            fallback_native: None,
            vtables: Vec::new(),
            green_thread_ctx: None,
        }
    }
}

impl Default for Program {
    fn default() -> Self {
        Self::new()
    }
}

/// Scope for REPL - persistent state across multiple executions
#[derive(Clone, Default)]
pub struct Scope {
    pub locals: HashMap<String, Value>,
    pub classes: HashMap<String, Class>,
    pub functions: HashMap<String, Function>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            locals: HashMap::new(),
            classes: HashMap::new(),
            functions: HashMap::new(),
        }
    }
}

pub struct VM {
    /// Loaded program data
    pub program: Program,
    /// Execution context (registers, stack, etc.)
    pub context: Context,
    /// Pending native methods to be attached to classes
    pending_native_methods: HashMap<String, HashMap<String, NativeFn>>,
    /// Pending class native_create callbacks
    pending_class_native_create: HashMap<String, NativeFn>,
    /// Pending class native_destroy callbacks
    pending_class_native_destroy: HashMap<String, NativeFn>,
    /// Breakpoints for debugging
    pub breakpoints: HashSet<(String, usize)>,
    /// Whether debugging mode is enabled
    pub is_debugging: bool,
    /// Opcode dispatch table for fast execution
    dispatch_table: [OpcodeHandler; 256],
}

/// VTable entry for interface method dispatch
#[derive(Clone, Debug)]
pub struct VTable {
    pub class_name: String,
    pub methods: Vec<String>,  // Ordered list of method names (by vtable index)
}

#[derive(Clone)]
struct ExceptionHandler {
    catch_pc: usize,
    catch_register: usize,
    call_stack_depth: usize,
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

impl VM {
    /// Create a new VM instance
    ///
    /// The registry-based VM uses a fixed register file size.
    /// Each call frame gets a window into this register file.
    pub fn new() -> Self {
        Self {
            program: Program::new(),
            context: Context::new(),
            pending_native_methods: HashMap::new(),
            pending_class_native_create: HashMap::new(),
            pending_class_native_destroy: HashMap::new(),
            breakpoints: HashSet::new(),
            is_debugging: false,
            dispatch_table: VM::create_dispatch_table(),
        }
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        if self.program.native_registry.get_index(name).is_some() {
            self.program.native_registry.hot_swap(name, f);
        } else {
            self.program.native_registry.register(name, f);
        }
    }

    pub fn register_native_method(&mut self, class_name: &str, method_name: &str, f: NativeFn) {
        self.pending_native_methods
            .entry(class_name.to_string())
            .or_insert_with(HashMap::new)
            .insert(method_name.to_string(), f);
    }

    pub fn register_class_native_create(&mut self, class_name: &str, f: NativeFn) {
        self.pending_class_native_create.insert(class_name.to_string(), f);
    }

    pub fn register_class_native_destroy(&mut self, class_name: &str, f: NativeFn) {
        self.pending_class_native_destroy.insert(class_name.to_string(), f);
    }

    pub fn native_method(&mut self, _class_name: &str, method_name: &str, func: NativeFn) -> NativeFunctionBuilder {
        NativeFunctionBuilder::new(method_name, func)
    }

    pub fn native(&mut self, name: &str, func: NativeFn) -> NativeFunctionBuilder {
        NativeFunctionBuilder::new(name, func)
    }

    pub fn module(&mut self, name: &str) -> NativeModule {
        NativeModule::new(name)
    }

    pub fn register_module(&mut self, module: NativeModule) {
        module.register(self);
    }

    pub fn register_fallback(&mut self, f: NativeFallbackFn) {
        self.program.fallback_native = Some(crate::linker::NativeFnType::Fallback(f));
        self.program.native_registry.set_fallback(f);
    }

    /// Load bytecode and initialize the VM
    pub fn load(&mut self, bytecode: &[u8], strings: Vec<String>, classes: Vec<Class>, functions: Vec<Function>, vtables: Vec<VTable>) -> Result<(), String> {
        self.program.bytecode = Arc::new(bytecode.to_vec());
        self.program.strings = Arc::new(strings);
        self.program.vtables = vtables;

        let mut classes_map = HashMap::new();
        for mut class in classes {
            // Try to find pending native methods by exact class name first
            let mut methods_added = false;
            if let Some(methods) = self.pending_native_methods.get(&class.name) {
                for (method_name, func) in methods {
                    class.native_methods.insert(method_name.clone(), *func);
                }
                methods_added = true;
            }
            
            // If not found, try to find by simple class name (last component)
            // This handles the case where native classes are registered with simple names
            // but bytecode uses qualified names
            if !methods_added {
                if let Some(simple_name) = class.name.split('.').last() {
                    if simple_name != class.name {
                        if let Some(methods) = self.pending_native_methods.get(simple_name) {
                            for (method_name, func) in methods {
                                class.native_methods.insert(method_name.clone(), *func);
                            }
                        }
                    }
                }
            }
            
            // Try to find native_create by exact class name first
            if let Some(on_init) = self.pending_class_native_create.get(&class.name) {
                class.native_create = Some(*on_init);
            } else if let Some(simple_name) = class.name.split('.').last() {
                if simple_name != class.name {
                    if let Some(on_init) = self.pending_class_native_create.get(simple_name) {
                        class.native_create = Some(*on_init);
                    }
                }
            }
            
            // Try to find native_destroy by exact class name first
            if let Some(on_destroy) = self.pending_class_native_destroy.get(&class.name) {
                class.native_destroy = Some(*on_destroy);
            } else if let Some(simple_name) = class.name.split('.').last() {
                if simple_name != class.name {
                    if let Some(on_destroy) = self.pending_class_native_destroy.get(simple_name) {
                        class.native_destroy = Some(*on_destroy);
                    }
                }
            }
            
            classes_map.insert(class.name.clone(), class);
        }
        self.program.classes = Arc::new(classes_map);

        let mut functions_map = HashMap::new();
        for function in functions {
            functions_map.insert(function.name.clone(), function);
        }
        self.program.functions = Arc::new(functions_map);

        // Initialize for execution
        self.set_pc(0);
        self.context.registers.fill(Value::Null);

        // Create initial call frame for module-level code
        let source_file = self.current_source_file();
        self.context.call_stack = vec![CallFrame::new(
            0,
            0,
            0,
            16,
            "<main>".to_string(),
            source_file,
            self.program.bytecode.clone(),
            self.program.strings.clone(),
            self.program.functions.clone(),
            self.program.classes.clone(),
            None,
        )];

        Ok(())
    }

    /// Start execution at a specific function
    /// This is used for green thread spawning
    pub fn call_function(&mut self, function_name: &str, args: Vec<Value>) -> Result<(), String> {
        let function = self.program.functions.get(function_name)
            .ok_or_else(|| format!("Function not found: {}", function_name))?;

        // Calculate frame base - start at beginning of registers for a new thread
        let new_frame_base = 0;

        if new_frame_base + function.register_count as usize > self.context.registers.len() {
            return Err("Register overflow: function requires too many registers".to_string());
        }

        // Create a new call frame for this function
        let new_frame = CallFrame::new(
            0,  // PC starts at 0 in function's bytecode
            new_frame_base,
            function.param_count,
            function.register_count,
            function_name.to_string(),
            function.source_file.clone(),
            Arc::new(function.bytecode.clone()),
            self.program.strings.clone(),
            self.program.functions.clone(),
            self.program.classes.clone(),
            None,  // No return register for top-level spawned function
        );

        self.push_frame(new_frame);

        // Set up arguments in the callee's frame (R1, R2, etc.)
        for (i, arg) in args.into_iter().enumerate() {
            self.set_reg((i + 1) as u8, arg);
        }

        Ok(())
    }

    /// Set a local variable by name
    pub fn set_local(&mut self, name: &str, value: Value) {
        self.context.locals.insert(name.to_string(), value);
    }

    /// Get a local variable by name
    pub fn get_local(&self, name: &str) -> Option<&Value> {
        self.context.locals.get(name)
    }

    /// Get current PC from the top frame
    #[inline]
    fn pc(&self) -> usize {
        self.context.call_stack.last().map(|f| f.pc).unwrap_or(0)
    }

    /// Set current PC in the top frame
    #[inline]
    fn set_pc(&mut self, pc: usize) {
        if let Some(frame) = self.context.call_stack.last_mut() {
            frame.pc = pc;
        }
    }

    /// Get current frame base
    #[inline]
    fn frame_base(&self) -> usize {
        self.context.call_stack.last().map(|f| f.frame_base).unwrap_or(0)
    }

    /// Read a register relative to the current frame base
    #[inline]
    fn get_reg(&self, offset: u8) -> &Value {
        let idx = self.frame_base() + offset as usize;
        &self.context.registers[idx]
    }

    /// Write to a register relative to the current frame base
    #[inline]
    fn set_reg(&mut self, offset: u8, value: Value) {
        let idx = self.frame_base() + offset as usize;
        self.context.registers[idx] = value;
    }

    /// Clone a register value
    #[inline]
    fn clone_reg(&mut self, dest: u8, src: u8) {
        let value = self.get_reg(src).clone();
        self.set_reg(dest, value);
    }

    /// Get the current source file from the top frame
    #[inline]
    fn current_source_file(&self) -> Option<String> {
        self.context.call_stack.last().and_then(|f| f.source_file.clone())
    }

    /// Get the current line number from the top frame
    #[inline]
    fn current_line(&self) -> usize {
        self.context.call_stack.last().map(|f| f.line_number).unwrap_or(1)
    }

    pub fn set_source_file(&mut self, file: &str) {
        if let Some(frame) = self.context.call_stack.last_mut() {
            frame.source_file = Some(file.to_string());
        }
    }

    pub fn set_line(&mut self, line: usize) {
        if let Some(frame) = self.context.call_stack.last_mut() {
            frame.line_number = line;
        }
    }

    pub fn get_line(&self) -> usize {
        self.current_line()
    }

    pub fn get_source_file(&self) -> Option<String> {
        self.current_source_file()
    }

    /// Set a breakpoint in a source file at a specific line
    pub fn set_breakpoint(&mut self, source_file: &str, line: usize) -> Result<(), String> {
        self.breakpoints.insert((source_file.to_string(), line));
        Ok(())
    }

    fn push_frame(&mut self, frame: CallFrame) {
        self.program.bytecode = frame.bytecode.clone();
        self.program.strings = frame.strings.clone();
        self.program.functions = frame.functions.clone();
        self.program.classes = frame.classes.clone();
        self.context.call_stack.push(frame);
    }

    fn pop_frame(&mut self) -> Option<CallFrame> {
        let frame = self.context.call_stack.pop();
        if let Some(top) = self.context.call_stack.last() {
            self.program.bytecode = top.bytecode.clone();
            self.program.strings = top.strings.clone();
            self.program.functions = top.functions.clone();
            self.program.classes = top.classes.clone();
        }
        frame
    }

    pub fn run(&mut self) -> Result<RunResult, Value> {
        if self.context.call_stack.is_empty() {
            return Ok(RunResult::Finished(Some(self.get_reg(0).clone())));
        }

        if self.pc() >= self.program.bytecode.len() {
            if self.context.call_stack.len() > 1 {
                let frame = self.pop_frame().unwrap();
                if let Some(rd) = frame.return_reg {
                    self.set_reg(rd, Value::Null);
                }
                return Ok(RunResult::InProgress);
            }
            return Ok(RunResult::Finished(Some(self.get_reg(0).clone())));
        }

        let opcode = self.program.bytecode[self.pc()];
        let result = match self.execute(opcode) {
            Ok(res) => res,
            Err(e) => {
                let exception = match &e {
                    Value::Exception(existing) => existing.clone(),
                    _ => self.build_exception(&e),
                };

                let mut has_local_handler = false;
                if let Some(handler) = self.context.exception_handlers.last() {
                    if handler.call_stack_depth == self.context.call_stack.len() {
                        has_local_handler = true;
                    }
                }

                if has_local_handler {
                    let handler = self.context.exception_handlers.pop().unwrap();
                    self.set_pc(handler.catch_pc);
                    self.set_reg(handler.catch_register as u8, Value::Exception(exception));
                    return Ok(RunResult::InProgress);
                } else {
                    return Err(Value::Exception(exception));
                }
            }
        };

        if opcode == Opcode::Halt as u8 {
            return Ok(RunResult::Finished(Some(self.get_reg(0).clone())));
        }

        match result {
            ExecutionResult::Breakpoint => Ok(RunResult::Breakpoint),
            ExecutionResult::Suspended => Ok(RunResult::Suspended),
            ExecutionResult::Continue => Ok(RunResult::InProgress),
        }
    }

    /// Set the callback for a pending native operation
    pub fn set_pending_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(Result<Value, Value>) + Send + 'static,
    {
        self.context.pending_native_callback = Some(Box::new(callback));
    }

    /// Resume execution after a native callback with the result
    pub fn resume_with_result(&mut self, result: Result<Value, Value>) -> Result<RunResult, Value> {
        debug_vm!("resume_with_result: PC = {}, bytecode.len() = {}", self.pc(), self.program.bytecode.len());
        if let Some(reg) = self.context.pending_native_result.take() {
            debug_vm!("resume_with_result: Setting reg {} with result", reg);
            match result {
                Ok(val) => self.set_reg(reg, val),
                Err(val) => self.set_reg(reg, val),
            }
        }
        // Continue execution
        debug_vm!("resume_with_result: Calling self.run()");
        self.run()
    }

    /// Check if VM is suspended
    pub fn is_suspended(&self) -> bool {
        self.context.pending_native_result.is_some()
    }

    fn build_exception(&self, value: &Value) -> Exception {
        let message = match value {
            Value::String(s) => s.clone(),
            Value::Exception(e) => e.message.clone(),
            _ => value.to_string(),
        };

        let stack_trace: Vec<StackFrame> = self.context.call_stack.iter().map(|frame| {
            StackFrame {
                function_name: frame.function_name.clone(),
                source_file: frame.source_file.clone(),
                line_number: Some(frame.line_number),
            }
        }).collect();

        Exception::new(message, stack_trace)
    }

    /// Execute a single opcode using the dispatch table
    #[inline]
    fn execute(&mut self, opcode: u8) -> Result<ExecutionResult, Value> {
        let handler = self.dispatch_table[opcode as usize];
        handler(self)
    }
}

#[derive(Debug, Clone)]
pub enum PromiseState {
    Pending,
    Resolved(Value),
    Rejected(Value),
}

#[derive(Debug, Clone)]
pub enum RunResult {
    Finished(Option<Value>),
    Breakpoint,
    Suspended,
    InProgress,
}

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Continue,
    Breakpoint,
    Suspended,
}

/// Opcode handler function type
/// Takes a mutable reference to VM and returns Result<ExecutionResult, Value>
pub type OpcodeHandler = fn(&mut VM) -> Result<ExecutionResult, Value>;

impl Serialize for Value {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            Value::String(s) => serializer.serialize_str(s),
            Value::Int8(i) => serializer.serialize_i8(*i),
            Value::Int16(i) => serializer.serialize_i16(*i),
            Value::Int32(i) => serializer.serialize_i32(*i),
            Value::Int64(i) => serializer.serialize_i64(*i),
            Value::UInt8(i) => serializer.serialize_u8(*i),
            Value::UInt16(i) => serializer.serialize_u16(*i),
            Value::UInt32(i) => serializer.serialize_u32(*i),
            Value::UInt64(i) => serializer.serialize_u64(*i),
            Value::Float32(f) => serializer.serialize_f32(*f),
            Value::Float64(f) => serializer.serialize_f64(*f),
            Value::Bool(b) => serializer.serialize_bool(*b),
            Value::Null => serializer.serialize_none(),
            Value::Instance(inst) => {
                let inst = inst.lock().unwrap();
                let mut map = serializer.serialize_map(Some(inst.fields.len()))?;
                for (k, v) in &inst.fields {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Value::Array(arr) => {
                use serde::ser::SerializeSeq;
                let elements = arr.lock().unwrap();
                let mut seq = serializer.serialize_seq(Some(elements.len()))?;
                for el in elements.iter() {
                    seq.serialize_element(el)?;
                }
                seq.end()
            }
            Value::Exception(e) => serializer.serialize_str(&e.message),
            Value::Promise(_) => serializer.serialize_str("[promise]"),
        }
    }
}

impl<'de> Deserialize<'de> for Value {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ValueVisitor;

        impl<'de> Visitor<'de> for ValueVisitor {
            type Value = Value;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a valid Bengal value")
            }

            fn visit_bool<E>(self, value: bool) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Bool(value))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Int64(value))
            }

            fn visit_u64<E>(self, value: u64) -> Result<Value, E>
            where E: serde::de::Error {
                if value <= i64::MAX as u64 {
                    Ok(Value::Int64(value as i64))
                } else {
                    Ok(Value::UInt64(value))
                }
            }

            fn visit_f64<E>(self, value: f64) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Float64(value))
            }

            fn visit_str<E>(self, value: &str) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::String(value.to_string()))
            }

            fn visit_string<E>(self, value: String) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::String(value))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut elements = Vec::new();
                while let Some(el) = seq.next_element()? {
                    elements.push(el);
                }
                Ok(Value::Array(Arc::new(Mutex::new(elements))))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
            where A: MapAccess<'de> {
                let mut fields = HashMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    fields.insert(key, value);
                }
                Ok(Value::Instance(Arc::new(Mutex::new(Instance {
                    class: "Object".to_string(),
                    fields,
                    private_fields: HashSet::new(),
                    native_data: Arc::new(Mutex::new(None)),
                }))))
            }

            fn visit_none<E>(self) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Null)
            }

            fn visit_unit<E>(self) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Null)
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}

/// Snapshot of VM state for REPL rollback support
#[derive(Clone)]
pub struct Snapshot {
    pub locals: HashMap<String, Value>,
    pub classes: HashMap<String, Class>,
    pub functions: HashMap<String, Function>,
}

impl VM {
    /// Create a snapshot of the current VM state
    pub fn snapshot(&self) -> Snapshot {
        Snapshot {
            locals: self.context.locals.clone(),
            classes: (*self.program.classes).clone(),
            functions: (*self.program.functions).clone(),
        }
    }

    /// Restore the VM to a previous state
    pub fn restore(&mut self, state: &Snapshot) {
        self.context.locals = state.locals.clone();
        self.program.classes = Arc::new(state.classes.clone());
        self.program.functions = Arc::new(state.functions.clone());
    }

    /// Create the opcode dispatch table
    fn create_dispatch_table() -> [OpcodeHandler; 256] {
        let mut table: [OpcodeHandler; 256] = [VM::op_unknown; 256];

        table[Opcode::Nop as usize] = VM::op_nop;
        table[Opcode::LoadConst as usize] = VM::op_load_const;
        table[Opcode::LoadInt as usize] = VM::op_load_int;
        table[Opcode::LoadFloat as usize] = VM::op_load_float;
        table[Opcode::LoadBool as usize] = VM::op_load_bool;
        table[Opcode::LoadNull as usize] = VM::op_load_null;
        table[Opcode::Move as usize] = VM::op_move;
        table[Opcode::LoadLocal as usize] = VM::op_load_local;
        table[Opcode::StoreLocal as usize] = VM::op_store_local;
        table[Opcode::GetProperty as usize] = VM::op_get_property;
        table[Opcode::SetProperty as usize] = VM::op_set_property;
        table[Opcode::Call as usize] = VM::op_call;
        table[Opcode::CallNative as usize] = VM::op_call_native;
        table[Opcode::Invoke as usize] = VM::op_invoke;
        table[Opcode::Return as usize] = VM::op_return;
        table[Opcode::InvokeInterface as usize] = VM::op_invoke_interface;
        table[Opcode::CallNativeIndexed as usize] = VM::op_call_native_indexed;
        table[Opcode::Jump as usize] = VM::op_jump;
        table[Opcode::JumpIfTrue as usize] = VM::op_jump_if_true;
        table[Opcode::JumpIfFalse as usize] = VM::op_jump_if_false;
        table[Opcode::Equal as usize] = VM::op_equal;
        table[Opcode::NotEqual as usize] = VM::op_not_equal;
        table[Opcode::Greater as usize] = VM::op_greater;
        table[Opcode::Less as usize] = VM::op_less;
        table[Opcode::GreaterEqual as usize] = VM::op_greater_equal;
        table[Opcode::LessEqual as usize] = VM::op_less_equal;
        table[Opcode::And as usize] = VM::op_and;
        table[Opcode::Or as usize] = VM::op_or;
        table[Opcode::Not as usize] = VM::op_not;
        table[Opcode::Add as usize] = VM::op_add;
        table[Opcode::Subtract as usize] = VM::op_subtract;
        table[Opcode::Multiply as usize] = VM::op_multiply;
        table[Opcode::Divide as usize] = VM::op_divide;
        table[Opcode::Modulo as usize] = VM::op_modulo;
        table[Opcode::BitAnd as usize] = VM::op_bit_and;
        table[Opcode::BitOr as usize] = VM::op_bit_or;
        table[Opcode::BitXor as usize] = VM::op_bit_xor;
        table[Opcode::BitNot as usize] = VM::op_bit_not;
        table[Opcode::ShiftLeft as usize] = VM::op_shift_left;
        table[Opcode::ShiftRight as usize] = VM::op_shift_right;
        table[Opcode::Concat as usize] = VM::op_concat;
        table[Opcode::Convert as usize] = VM::op_convert;
        table[Opcode::Array as usize] = VM::op_array;
        table[Opcode::Index as usize] = VM::op_index;
        table[Opcode::Line as usize] = VM::op_line;
        table[Opcode::TryStart as usize] = VM::op_try_start;
        table[Opcode::TryEnd as usize] = VM::op_try_end;
        table[Opcode::Throw as usize] = VM::op_throw;
        table[Opcode::Yield as usize] = VM::op_yield;
        table[Opcode::Spawn as usize] = VM::op_spawn;
        table[Opcode::Breakpoint as usize] = VM::op_breakpoint;
        table[Opcode::Halt as usize] = VM::op_halt;

        table
    }

    #[inline]
    fn op_nop(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_const(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let idx = ((self.program.bytecode[self.pc() + 1] as usize) << 8) | (self.program.bytecode[self.pc()] as usize);
        self.set_pc(self.pc() + 2);
        let s = self.program.strings.get(idx)
            .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
            .clone();
        self.set_reg(rd, Value::String(s));
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_int(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let bytes: [u8; 8] = self.program.bytecode[self.pc()..self.pc() + 8]
            .try_into()
            .map_err(|_| Value::String("Invalid int encoding".to_string()))?;
        let n = i64::from_le_bytes(bytes);
        self.set_reg(rd, Value::Int64(n));
        self.set_pc(self.pc() + 8);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_float(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let bytes: [u8; 8] = self.program.bytecode[self.pc()..self.pc() + 8]
            .try_into()
            .map_err(|_| Value::String("Invalid float encoding".to_string()))?;
        let n = f64::from_le_bytes(bytes);
        self.set_reg(rd, Value::Float64(n));
        self.set_pc(self.pc() + 8);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_bool(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let b = self.program.bytecode[self.pc()] != 0;
        self.set_reg(rd, Value::Bool(b));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_null(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Null);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_move(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        self.clone_reg(rd, rs);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_load_local(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let idx = self.program.bytecode[self.pc()] as usize;
        let name = self.program.strings.get(idx)
            .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
            .clone();
        let value = self.context.locals.get(&name).cloned().unwrap_or(Value::Null);
        self.set_reg(rd, value);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_store_local(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        let name = self.program.strings.get(idx)
            .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
            .clone();
        let value = self.get_reg(rs).clone();
        self.context.locals.insert(name, value);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_get_property(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let robj = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let idx = self.program.bytecode[self.pc()] as usize;
        let name = self.program.strings.get(idx)
            .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
            .clone();

        let robj_val = self.get_reg(robj).clone();
        match robj_val {
            Value::Instance(instance) => {
                let instance_lock = instance.lock().unwrap();
                let value = instance_lock.fields.get(&name).cloned().unwrap_or(Value::Null);
                self.set_reg(rd, value);
            }
            Value::Exception(exception) => {
                if name == "message" {
                    self.set_reg(rd, Value::String(exception.message.clone()));
                } else if name == "stack_trace" {
                    let trace = exception.stack_trace.iter().map(|f| f.to_string()).collect::<Vec<String>>().join("\n");
                    self.set_reg(rd, Value::String(trace));
                } else {
                    self.set_reg(rd, Value::Null);
                }
            }
            Value::Array(arr) => {
                if name == "length" {
                    let elements = arr.lock().unwrap();
                    self.set_reg(rd, Value::Int64(elements.len() as i64));
                } else {
                    return Err(Value::String(format!("Unknown property '{}' on Array", name)));
                }
            }
            _ => {
                return Err(Value::String(format!("Expected instance for property get, got {:?}", robj_val)));
            }
        }
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_set_property(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let robj = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        let name = self.program.strings.get(idx)
            .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
            .clone();

        let value = self.get_reg(rs).clone();
        let instance = if let Value::Instance(instance) = self.get_reg(robj) {
            instance.clone()
        } else {
            return Err(Value::String("Expected instance for property set".to_string()));
        };

        let mut instance_lock = instance.lock().unwrap();
        instance_lock.fields.insert(name, value);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_call(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let func_idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);

        let func_name_raw = self.program.strings.get(func_idx)
            .ok_or_else(|| Value::String(format!("Invalid function index: {}", func_idx)))?
            .clone();

        let mut function_opt = self.program.functions.get(&func_name_raw).cloned();

        if function_opt.is_none() {
            if !func_name_raw.contains('.') {
                if let Some(ref source) = self.current_source_file() {
                    let file_name = std::path::Path::new(source)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("");

                    let qualified = format!("{}.{}", file_name, func_name_raw);
                    function_opt = self.program.functions.get(&qualified).cloned();
                }
            }
        }

        if function_opt.is_none() {
            let search_name = func_name_raw.clone();
            for (name, func) in self.program.functions.as_ref() {
                if name == &search_name || name.ends_with(&format!(".{}", search_name)) {
                    function_opt = Some(func.clone());
                    break;
                }
            }
        }

        if function_opt.is_none() {
            if let Some(paren_pos) = func_name_raw.find('(') {
                let base_name = &func_name_raw[..paren_pos];
                for (name, func) in self.program.functions.as_ref() {
                    if name == base_name || name.ends_with(&format!(".{}", base_name)) {
                        function_opt = Some(func.clone());
                        break;
                    }
                }
            }
        }

        let func_name = if let Some(ref f) = function_opt {
            f.name.clone()
        } else {
            func_name_raw.clone()
        };

        let base_class_name = extract_base_class_name(&func_name);
        let is_constructor = func_name.contains(".constructor(");
        
        // Also check if func_name ends with "()" and matches a class name (e.g., "std.http.HttpClient()")
        let potential_class = if func_name.ends_with("()") {
            Some(&func_name[..func_name.len()-2]) // Remove "()"
        } else {
            None
        };
        let is_class_instantiation = !is_constructor && potential_class.map_or(false, |pc| {
            self.program.classes.contains_key(pc)
        });

        // Handle constructor calls (both bytecode and native)
        if is_constructor || is_class_instantiation {
            let class_name: String = if is_class_instantiation {
                func_name[..func_name.len()-2].to_string() // Remove "()"
            } else {
                base_class_name.to_string()
            };

            if let Some(class) = self.program.classes.get(&class_name).cloned() {
                let instance = Value::Instance(Arc::new(Mutex::new(Instance {
                    class: class_name.to_string(),
                    fields: class.fields.clone(),
                    private_fields: class.private_fields.clone(),
                    native_data: Arc::new(Mutex::new(None)),
                })));
                self.set_reg(rd, instance.clone());

                // Call native_create if available
                if let Some(native_create) = class.native_create {
                    let mut args = vec![instance.clone()];
                    let _ = native_create(&mut args);
                }

                // Check for native constructor method
                // Extract method name from "ClassName.constructor()" -> "constructor()"
                if let Some(paren_pos) = func_name.find(".constructor(") {
                    let method_name = &func_name[paren_pos + 1..]; // "constructor()"
                    if let Some(native_ctor) = class.native_methods.get(method_name) {
                        let mut args = vec![instance];
                        let _ = native_ctor(&mut args);
                        // Don't overwrite rd - it already has the instance
                        return Ok(ExecutionResult::Continue);
                    }
                }

                // For bytecode constructors, continue to function call below
                // For native-only constructors, we're done
                if class.native_create.is_some() && class.methods.is_empty() {
                    return Ok(ExecutionResult::Continue);
                }

                // If native_create was called and there's no native constructor method,
                // we're done (the instance was created by native_create)
                if class.native_create.is_some() {
                    return Ok(ExecutionResult::Continue);
                }
            }
        }

        let native_idx = self.program.native_registry.get_index(&func_name)
            .or_else(|| self.program.native_registry.get_index_by_prefix(&func_name));

        if let Some(idx) = native_idx {
            if let Some(func_type) = self.program.native_registry.get_by_index(idx) {
                let mut args = Vec::new();
                for i in 0..arg_count {
                    args.push(self.get_reg(arg_start + i).clone());
                }
                let result = match func_type {
                    crate::linker::NativeFnType::Sync(f) => f(&mut args),
                    crate::linker::NativeFnType::Async(_f) => NativeResult::Pending,
                    crate::linker::NativeFnType::Fallback(f) => f(&func_name, &mut args),
                };
                match result {
                    NativeResult::Ready(val) => {
                        self.set_reg(rd, val);
                        return Ok(ExecutionResult::Continue);
                    }
                    NativeResult::Pending => {
                        self.context.pending_native_result = Some(rd);
                        return Ok(ExecutionResult::Suspended);
                    }
                    NativeResult::PendingWithWaitId(wait_id) => {
                        self.context.pending_native_result = Some(rd);
                        self.context.pending_wait_id = Some(wait_id);
                        return Ok(ExecutionResult::Suspended);
                    }
                }
            } else {
                return Err(Value::String(format!("Native function not found: {}", func_name)));
            }
        }

        if let Some(function) = function_opt {
            let mut args = Vec::new();

            // For constructors, the instance is created by the VM and stored in rd
            // For instance methods, self is passed as the first argument from the caller
            if is_constructor {
                args.push(self.get_reg(rd).clone());
            }

            // Collect all arguments from the caller's frame
            for i in 0..arg_count {
                args.push(self.get_reg(arg_start + i).clone());
            }

            // Calculate new frame base: start after the caller's register window
            // This prevents register overlap between caller and callee
            let caller_frame = self.context.call_stack.last().unwrap();
            let new_frame_base = caller_frame.frame_base + caller_frame.register_count as usize;

            if new_frame_base + function.register_count as usize > self.context.registers.len() {
                return Err(Value::String("Register overflow: too many nested calls".to_string()));
            }

            let new_frame = CallFrame::new(
                0,
                new_frame_base,
                function.param_count,
                function.register_count,
                func_name.clone(),
                function.source_file.clone(),
                Arc::new(function.bytecode.clone()),
                self.program.strings.clone(),
                self.program.functions.clone(),
                self.program.classes.clone(),
                Some(rd),
            );

            self.push_frame(new_frame);

            // Set up arguments in the callee's frame
            // R1 = first arg (self for instance methods/constructors), R2 = second arg, etc.
            for (i, arg) in args.iter().enumerate() {
                self.set_reg((i + 1) as u8, arg.clone());
            }

            return Ok(ExecutionResult::Continue);
        } else {
            // Check if this is a method call in the format "Class.method(args)" (new)
            // or "Class_method(args)" (old format for backward compatibility)
            // For qualified class names like "std.http.HttpClient.get(str)", we need to find
            // the last dot before the method name (which starts with a lowercase letter)
            let (potential_class_name, method_name_with_args) =
                if let Some(paren_pos) = func_name_raw.find('(') {
                    // Find the last dot before the opening parenthesis
                    let before_paren = &func_name_raw[..paren_pos];
                    if let Some(dot_pos) = before_paren.rfind('.') {
                        // Check if what follows the dot looks like a method name (starts with lowercase)
                        let after_dot = &func_name_raw[dot_pos + 1..];
                        if after_dot.chars().next().map_or(false, |c| c.is_lowercase()) {
                            // Format: Class.method(args)
                            (&func_name_raw[..dot_pos], &func_name_raw[dot_pos + 1..])
                        } else {
                            ("", func_name_raw.as_str())
                        }
                    } else {
                        ("", func_name_raw.as_str())
                    }
                } else if let Some(underscore_pos) = func_name_raw.find('_') {
                    // Old format: Class_method(args)
                    (&func_name_raw[..underscore_pos], &func_name_raw[underscore_pos + 1..])
                } else {
                    ("", func_name_raw.as_str())
                };

            // Check if this looks like a class method call (last component of class name starts with uppercase)
            let looks_like_class = potential_class_name.split('.').last()
                .map_or(false, |last| last.chars().next().map_or(false, |c| c.is_uppercase()));
            
            if !potential_class_name.is_empty() && looks_like_class {
                // First try direct lookup (for simple class names or when using fully qualified name)
                // If not found, try to find a class that ends with the simple name (e.g., "HttpClient" matches "std.http.HttpClient")
                let class_opt = self.program.classes.get(potential_class_name).cloned()
                    .or_else(|| {
                        self.program.classes.iter()
                            .find(|(name, _)| name.ends_with(&format!(".{}", potential_class_name)))
                            .map(|(_, class)| class.clone())
                    });

                if let Some(class) = class_opt {
                    // Look for native method
                    if let Some(native_method) = class.native_methods.get(method_name_with_args) {
                        // Get self from rd (instance was created by previous call or constructor)
                        let instance = self.get_reg(rd).clone();
                        let mut args = vec![instance];
                        for i in 0..arg_count {
                            args.push(self.get_reg(arg_start + i).clone());
                        }
                        let result = native_method(&mut args);
                        if let NativeResult::Ready(val) = result {
                            self.set_reg(rd, val);
                        }
                        return Ok(ExecutionResult::Continue);
                    }

                    // Look for bytecode method
                    if let Some(method) = class.methods.get(method_name_with_args) {
                        // Create call frame for bytecode method
                        // Get self from arg_start (first argument is the instance)
                        let mut args = vec![self.get_reg(arg_start).clone()]; // self
                        for i in 1..arg_count {
                            args.push(self.get_reg(arg_start + i).clone());
                        }

                        let caller_frame = self.context.call_stack.last().unwrap();
                        let new_frame_base = caller_frame.frame_base + caller_frame.register_count as usize;

                        if new_frame_base + method.register_count as usize > self.context.registers.len() {
                            return Err(Value::String("Register overflow: too many nested calls".to_string()));
                        }

                        // Method doesn't have param_count or source_file, use defaults
                        let new_frame = CallFrame::new(
                            0,
                            new_frame_base,
                            arg_count + 1, // self + args
                            method.register_count,
                            func_name_raw.clone(),
                            None,
                            Arc::new(method.bytecode.clone()),
                            self.program.strings.clone(),
                            self.program.functions.clone(),
                            self.program.classes.clone(),
                            Some(rd),
                        );

                        self.push_frame(new_frame);

                        // Set up arguments
                        for (i, arg) in args.iter().enumerate() {
                            self.set_reg((i + 1) as u8, arg.clone());
                        }

                        return Ok(ExecutionResult::Continue);
                    }
                }
            }

            return Err(Value::String(format!("Function not found: {}", func_name)));
        }
    }

    #[inline]
    fn op_call_native(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let name_idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;

        let name = self.program.strings.get(name_idx)
            .ok_or_else(|| Value::String(format!("Invalid native name index: {}", name_idx)))?
            .clone();

        // Check if this is a class method call in the format "Class.method(args)"
        // For qualified class names like "std.http.HttpClient.get(str)", we need to find
        // the last dot before the method name (which starts with a lowercase letter)
        let (class_name, method_name) = if let Some(paren_pos) = name.find('(') {
            let before_paren = &name[..paren_pos];
            if let Some(dot_pos) = before_paren.rfind('.') {
                let after_dot = &name[dot_pos + 1..];
                if after_dot.chars().next().map_or(false, |c| c.is_lowercase()) {
                    (&name[..dot_pos], &name[dot_pos + 1..])
                } else {
                    ("", name.as_str())
                }
            } else {
                ("", name.as_str())
            }
        } else {
            ("", name.as_str())
        };

        // Try class method lookup first
        // Check if class_name looks like a class (last component starts with uppercase)
        let looks_like_class = class_name.split('.').last()
            .map_or(false, |last| last.chars().next().map_or(false, |c| c.is_uppercase()));
        
        if !class_name.is_empty() && looks_like_class {
            // First try direct lookup, then try to find a class that ends with the class name
            let class_opt = self.program.classes.get(class_name).cloned()
                .or_else(|| {
                    self.program.classes.iter()
                        .find(|(n, _)| n.ends_with(&format!(".{}", class_name)))
                        .map(|(_, class)| class.clone())
                });

            if let Some(class) = class_opt {
                // Special handling for constructor - it should create the instance
                if method_name.starts_with("constructor(") {
                    // Create the instance
                    let instance = Value::Instance(Arc::new(Mutex::new(Instance {
                        class: class_name.to_string(),
                        fields: class.fields.clone(),
                        private_fields: class.private_fields.clone(),
                        native_data: Arc::new(Mutex::new(None)),
                    })));
                    self.set_reg(rd, instance.clone());

                    // Call native_create if available
                    if let Some(native_create) = class.native_create {
                        let mut args = vec![instance.clone()];
                        let _ = native_create(&mut args);
                    }

                    // Call the native constructor method if available
                    if let Some(native_ctor) = class.native_methods.get(method_name) {
                        let mut args = vec![instance];
                        let _ = native_ctor(&mut args);
                        // Don't overwrite rd - it already has the instance
                    }
                    self.set_pc(self.pc() + 1);
                    return Ok(ExecutionResult::Continue);
                }

                if let Some(native_method) = class.native_methods.get(method_name) {
                    // Get self from arg_start (first argument is the instance)
                    let instance = self.get_reg(arg_start).clone();
                    let mut args = vec![instance];
                    for i in 1..arg_count {
                        args.push(self.get_reg(arg_start + i).clone());
                    }
                    let result = native_method(&mut args);
                    match result {
                        NativeResult::Ready(val) => {
                            self.set_reg(rd, val);
                            self.set_pc(self.pc() + 1);
                            return Ok(ExecutionResult::Continue);
                        }
                        NativeResult::Pending => {
                            self.context.pending_native_result = Some(rd);
                            self.set_pc(self.pc() + 1);
                            return Ok(ExecutionResult::Suspended);
                        }
                        NativeResult::PendingWithWaitId(wait_id) => {
                            self.context.pending_native_result = Some(rd);
                            self.context.pending_wait_id = Some(wait_id);
                            self.set_pc(self.pc() + 1);
                            return Ok(ExecutionResult::Suspended);
                        }
                    }
                }
            }
        }

        let mut args = Vec::new();
        for i in 0..arg_count {
            args.push(self.get_reg(arg_start + i).clone());
        }

        let result = match self.program.native_registry.get_index(&name)
            .or_else(|| self.program.native_registry.get_index_by_prefix(&name))
            .and_then(|idx| self.program.native_registry.get_by_index(idx)) {
            Some(func_type) => {
                match func_type {
                    crate::linker::NativeFnType::Sync(f) => f(&mut args),
                    crate::linker::NativeFnType::Async(_f) => NativeResult::Pending,
                    crate::linker::NativeFnType::Fallback(f) => f(&name, &mut args),
                }
            }
            None => {
                match self.program.fallback_native.clone() {
                    Some(func_type) => {
                        match func_type {
                            crate::linker::NativeFnType::Sync(f) => f(&mut args),
                            crate::linker::NativeFnType::Async(_f) => NativeResult::Pending,
                            crate::linker::NativeFnType::Fallback(f) => f(&name, &mut args),
                        }
                    }
                    None => {
                        return Err(Value::String(format!("Native function not found: {}", name)));
                    }
                }
            }
        };

        match result {
            NativeResult::Ready(val) => {
                self.set_reg(rd, val);
                self.set_pc(self.pc() + 1);
            }
            NativeResult::Pending => {
                self.set_pc(self.pc() + 1);  // Advance past arg_count before suspending
                self.context.pending_native_result = Some(rd);
                return Ok(ExecutionResult::Suspended);
            }
            NativeResult::PendingWithWaitId(wait_id) => {
                self.set_pc(self.pc() + 1);  // Advance past arg_count before suspending
                self.context.pending_native_result = Some(rd);
                self.context.pending_wait_id = Some(wait_id);
                return Ok(ExecutionResult::Suspended);
            }
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_call_native_indexed(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let func_idx_lo = self.program.bytecode[self.pc()] as u16;
        self.set_pc(self.pc() + 1);
        let func_idx_hi = self.program.bytecode[self.pc()] as u16;
        self.set_pc(self.pc() + 1);
        let func_index = (func_idx_hi << 8) | func_idx_lo;

        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;

        let mut args = Vec::new();
        for i in 0..arg_count {
            args.push(self.get_reg(arg_start + i).clone());
        }

        let result = match self.program.native_registry.get_by_index(func_index) {
            Some(func_type) => {
                match func_type {
                    crate::linker::NativeFnType::Sync(f) => f(&mut args),
                    crate::linker::NativeFnType::Async(_f) => NativeResult::Pending,
                    crate::linker::NativeFnType::Fallback(f) => {
                        // For indexed calls, try to get the function name from the registry
                        let name = self.program.native_registry.get_name_by_index(func_index)
                            .unwrap_or_else(|| format!("unknown@{}", func_index));
                        f(&name, &mut args)
                    },
                }
            }
            None => {
                match self.program.fallback_native.clone() {
                    Some(func_type) => {
                        match func_type {
                            crate::linker::NativeFnType::Sync(f) => f(&mut args),
                            crate::linker::NativeFnType::Async(_f) => NativeResult::Pending,
                            crate::linker::NativeFnType::Fallback(f) => {
                                // For indexed calls, try to get the function name from the registry
                                let name = self.program.native_registry.get_name_by_index(func_index)
                                    .unwrap_or_else(|| format!("unknown@{}", func_index));
                                f(&name, &mut args)
                            },
                        }
                    }
                    None => {
                        return Err(Value::String(format!("Native function not found at index: {}", func_index)));
                    }
                }
            }
        };

        match result {
            NativeResult::Ready(val) => {
                self.set_reg(rd, val);
                self.set_pc(self.pc() + 1);
            }
            NativeResult::Pending => {
                self.set_pc(self.pc() + 1);
                self.context.pending_native_result = Some(rd);
                return Ok(ExecutionResult::Suspended);
            }
            NativeResult::PendingWithWaitId(wait_id) => {
                self.set_pc(self.pc() + 1);
                self.context.pending_native_result = Some(rd);
                self.context.pending_wait_id = Some(wait_id);
                return Ok(ExecutionResult::Suspended);
            }
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_invoke(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let method_idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);

        let name = self.program.strings.get(method_idx)
            .ok_or_else(|| Value::String(format!("Invalid method index: {}", method_idx)))?
            .clone();

        let mut args = Vec::new();
        for i in 0..arg_count {
            args.push(self.get_reg(arg_start + i).clone());
        }

        // Handle array methods
        if let Some(Value::Array(_)) = args.first() {
            let base_name = if let Some(paren_pos) = name.find('(') {
                &name[..paren_pos]
            } else {
                &name
            };

            let result = match base_name {
                "length" => {
                    if let Value::Array(arr) = &args[0] {
                        let elements = arr.lock().unwrap();
                        Ok(Value::Int64(elements.len() as i64))
                    } else {
                        Err(Value::String("length requires an array".to_string()))
                    }
                }
                "add" => {
                    if args.len() < 2 {
                        Err(Value::String("add requires a value argument".to_string()))
                    } else if let Value::Array(arr) = &args[0] {
                        let mut elements = arr.lock().unwrap();
                        elements.push(args[1].clone());
                        Ok(Value::Null)
                    } else {
                        Err(Value::String("add requires an array".to_string()))
                    }
                }
                _ => Err(Value::String(format!("Method '{}' not found on Array", name))),
            };
            self.set_reg(rd, result?);
            self.set_pc(self.pc() + 1);
            return Ok(ExecutionResult::Continue);
        }

        // Handle string methods
        if let Some(Value::String(_)) = args.first() {
            let base_name = if let Some(paren_pos) = name.find('(') {
                &name[..paren_pos]
            } else {
                &name
            };

            let result = match base_name {
                "length" => {
                    if let Value::String(s) = &args[0] {
                        Ok(Value::Int64(s.len() as i64))
                    } else {
                        Err(Value::String("length requires a string".to_string()))
                    }
                }
                "trim" => {
                    if let Value::String(s) = &args[0] {
                        Ok(Value::String(s.trim().to_string()))
                    } else {
                        Err(Value::String("trim requires a string".to_string()))
                    }
                }
                "toInt" => {
                    if let Value::String(s) = &args[0] {
                        match s.parse::<i64>() {
                            Ok(n) => Ok(Value::Int64(n)),
                            Err(_) => Ok(Value::Null),
                        }
                    } else {
                        Err(Value::String("toInt requires a string".to_string()))
                    }
                }
                "toFloat" => {
                    if let Value::String(s) = &args[0] {
                        match s.parse::<f64>() {
                            Ok(n) => Ok(Value::Float64(n)),
                            Err(_) => Ok(Value::Null),
                        }
                    } else {
                        Err(Value::String("toFloat requires a string".to_string()))
                    }
                }
                "contains" => {
                    if args.len() < 2 {
                        Err(Value::String("contains requires a string argument".to_string()))
                    } else if let (Value::String(s), Value::String(substr)) = (&args[0], &args[1]) {
                        Ok(Value::Bool(s.contains(substr)))
                    } else {
                        Err(Value::String("contains requires string arguments".to_string()))
                    }
                }
                "startsWith" => {
                    if args.len() < 2 {
                        Err(Value::String("startsWith requires a string argument".to_string()))
                    } else if let (Value::String(s), Value::String(prefix)) = (&args[0], &args[1]) {
                        Ok(Value::Bool(s.starts_with(prefix)))
                    } else {
                        Err(Value::String("startsWith requires string arguments".to_string()))
                    }
                }
                "endsWith" => {
                    if args.len() < 2 {
                        Err(Value::String("endsWith requires a string argument".to_string()))
                    } else if let (Value::String(s), Value::String(suffix)) = (&args[0], &args[1]) {
                        Ok(Value::Bool(s.ends_with(suffix)))
                    } else {
                        Err(Value::String("endsWith requires string arguments".to_string()))
                    }
                }
                "substring" => {
                    if args.len() < 3 {
                        Err(Value::String("substring requires start and end arguments".to_string()))
                    } else if let (Value::String(s), Value::Int64(start), Value::Int64(end)) = (&args[0], &args[1], &args[2]) {
                        let start = *start as usize;
                        let end = *end as usize;
                        if start > s.len() || end > s.len() || start > end {
                            Err(Value::String("substring: invalid indices".to_string()))
                        } else {
                            Ok(Value::String(s[start..end].to_string()))
                        }
                    } else {
                        Err(Value::String("substring requires string and int arguments".to_string()))
                    }
                }
                "toLower" => {
                    if let Value::String(s) = &args[0] {
                        Ok(Value::String(s.to_lowercase()))
                    } else {
                        Err(Value::String("toLower requires a string".to_string()))
                    }
                }
                "toUpper" => {
                    if let Value::String(s) = &args[0] {
                        Ok(Value::String(s.to_uppercase()))
                    } else {
                        Err(Value::String("toUpper requires a string".to_string()))
                    }
                }
                "replace" => {
                    if args.len() < 3 {
                        Err(Value::String("replace requires pattern and replacement arguments".to_string()))
                    } else if let (Value::String(s), Value::String(pattern), Value::String(replacement)) = (&args[0], &args[1], &args[2]) {
                        Ok(Value::String(s.replace(pattern, replacement)))
                    } else {
                        Err(Value::String("replace requires string arguments".to_string()))
                    }
                }
                "split" => {
                    if args.len() < 2 {
                        Err(Value::String("split requires a delimiter argument".to_string()))
                    } else if let (Value::String(s), Value::String(delimiter)) = (&args[0], &args[1]) {
                        let elements: Vec<Value> = s.split(delimiter)
                            .map(|part| Value::String(part.to_string()))
                            .collect();
                        Ok(Value::Array(Arc::new(Mutex::new(elements))))
                    } else {
                        Err(Value::String("split requires string arguments".to_string()))
                    }
                }
                _ => Err(Value::String(format!("Method '{}' not found on str", name))),
            };
            self.set_reg(rd, result?);
            self.set_pc(self.pc() + 1);
            return Ok(ExecutionResult::Continue);
        }

        let instance = if let Some(Value::Instance(instance)) = args.first() {
            instance.clone()
        } else {
            return Err(Value::String("Invoke requires an instance".to_string()));
        };

        let class_name = instance.lock().unwrap().class.clone();
        if let Some(class) = self.program.classes.get(&class_name).cloned() {
            if let Some(native_method) = class.native_methods.get(&name) {
                let mut method_args = args.clone();
                let result = native_method(&mut method_args);
                match result {
                    NativeResult::Ready(val) => {
                        if name == "constructor" {
                            self.set_reg(rd, args.first().cloned().unwrap_or(Value::Null));
                        } else {
                            self.set_reg(rd, val);
                        }
                        self.set_pc(self.pc() + 1);
                    }
                    NativeResult::Pending => {
                        self.set_pc(self.pc() + 1);
                        self.context.pending_native_result = Some(rd);
                        return Ok(ExecutionResult::Suspended);
                    }
                    NativeResult::PendingWithWaitId(wait_id) => {
                        self.set_pc(self.pc() + 1);
                        self.context.pending_native_result = Some(rd);
                        self.context.pending_wait_id = Some(wait_id);
                        return Ok(ExecutionResult::Suspended);
                    }
                }
            } else {
                let method_opt = class.methods.get(&name);

                let method_opt = method_opt.or_else(|| {
                    let (base_name, requested_params) = if let Some(paren_pos) = name.find('(') {
                        let params_str = &name[paren_pos + 1..name.len() - 1];
                        (&name[..paren_pos], params_str.split(',').collect::<Vec<_>>())
                    } else {
                        (&name[..], Vec::new())
                    };

                    class.methods.iter().find(|(k, _)| {
                        if let Some(paren_pos) = k.find('(') {
                            let k_base = &k[..paren_pos];
                            let k_params_str = &k[paren_pos + 1..k.len() - 1];
                            let k_params: Vec<&str> = k_params_str.split(',').collect();

                            if k_base != base_name || k_params.len() != requested_params.len() {
                                return false;
                            }

                            for (k_param, req_param) in k_params.iter().zip(requested_params.iter()) {
                                let is_generic = k_param.len() == 1 && k_param.chars().next().unwrap().is_uppercase();
                                if !is_generic && k_param != req_param {
                                    return false;
                                }
                            }
                            true
                        } else {
                            false
                        }
                    }).map(|(_, v)| v)
                });

                if let Some(method) = method_opt {
                    // Calculate new frame base: start after the caller's register window
                    let caller_frame = self.context.call_stack.last().unwrap();
                    let new_frame_base = caller_frame.frame_base + caller_frame.register_count as usize;

                    if new_frame_base + method.register_count as usize > self.context.registers.len() {
                        return Err(Value::String("Register overflow in method call".to_string()));
                    }

                    let new_frame = CallFrame::new(
                        0,
                        new_frame_base,
                        arg_count,
                        method.register_count,
                        format!("{}.{}", class_name, name),
                        self.current_source_file(),
                        Arc::new(method.bytecode.clone()),
                        self.program.strings.clone(),
                        self.program.functions.clone(),
                        self.program.classes.clone(),
                        Some(rd),
                    );

                    self.push_frame(new_frame);

                    for (i, arg) in args.iter().enumerate() {
                        self.set_reg((i + 1) as u8, arg.clone());
                    }

                    return Ok(ExecutionResult::Continue);
                } else {
                    let base_name = if let Some(paren_pos) = name.find('(') {
                        &name[..paren_pos]
                    } else {
                        &name
                    };

                    let mut available_native: Vec<&String> = class.native_methods.keys()
                        .filter(|k| {
                            if let Some(paren_pos) = k.find('(') {
                                &k[..paren_pos] == base_name
                            } else {
                                k.as_str() == base_name
                            }
                        })
                        .collect();

                    let mut available_bytecode: Vec<&String> = class.methods.keys()
                        .filter(|k| {
                            if let Some(paren_pos) = k.find('(') {
                                &k[..paren_pos] == base_name
                            } else {
                                k.as_str() == base_name
                            }
                        })
                        .collect();

                    if !available_native.is_empty() || !available_bytecode.is_empty() {
                        let mut available = Vec::new();
                        available_native.sort();
                        available_bytecode.sort();
                        available.extend(available_native.iter().map(|s| s.as_str()));
                        available.extend(available_bytecode.iter().map(|s| s.as_str()));
                        return Err(Value::String(format!(
                            "Method '{}' not found on class '{}'. Available methods: [{}]",
                            name, class_name, available.join(", ")
                        )));
                    } else {
                        return Err(Value::String(format!("Method '{}' not found on class '{}'", name, class_name)));
                    }
                }
            }
        } else {
            return Err(Value::String(format!("Class '{}' not found", class_name)));
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_invoke_interface(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let method_idx = self.program.bytecode[self.pc()] as usize;
        self.set_pc(self.pc() + 1);
        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);

        let mut args = Vec::new();
        for i in 0..arg_count {
            args.push(self.get_reg(arg_start + i).clone());
        }

        let instance = if let Some(Value::Instance(instance)) = args.first() {
            instance.clone()
        } else {
            return Err(Value::String("InvokeInterface requires an instance".to_string()));
        };

        let class_name = instance.lock().unwrap().class.clone();

        if let Some(class) = self.program.classes.get(&class_name).cloned() {
            let method_name = class.vtable.get(method_idx)
                .ok_or_else(|| Value::String(format!(
                    "Vtable method index {} out of range for class '{}' (vtable size: {})",
                    method_idx, class_name, class.vtable.len()
                )))?;
            
            let method_opt = class.methods.get(method_name)
                .or_else(|| {
                    // Try to find a method that matches the vtable entry
                    // Methods are stored as "Class.method(args)" but vtable has "method"
                    class.methods.iter().find(|(k, _)| {
                        // Extract the method name part after the dot
                        if let Some(dot_pos) = k.rfind('.') {
                            let method_part = &k[dot_pos + 1..];
                            // Remove parentheses and parameters to get base method name
                            let base_method = if let Some(paren_pos) = method_part.find('(') {
                                &method_part[..paren_pos]
                            } else {
                                method_part
                            };
                            // Compare base method names
                            base_method == method_name
                        } else {
                            k.as_str() == method_name
                        }
                    }).map(|(_, v)| v)
                });

            if let Some(method) = method_opt {
                // Calculate new frame base: start after the caller's register window
                let caller_frame = self.context.call_stack.last().unwrap();
                let new_frame_base = caller_frame.frame_base + caller_frame.register_count as usize;

                if new_frame_base + method.register_count as usize > self.context.registers.len() {
                    return Err(Value::String("Register overflow in interface method call".to_string()));
                }

                let new_frame = CallFrame::new(
                    0,
                    new_frame_base,
                    arg_count,
                    method.register_count,
                    format!("{}.{}", class_name, method_name),
                    self.current_source_file(),
                    Arc::new(method.bytecode.clone()),
                    self.program.strings.clone(),
                    self.program.functions.clone(),
                    self.program.classes.clone(),
                    Some(rd),
                );

                self.push_frame(new_frame);

                for (i, arg) in args.iter().enumerate() {
                    self.set_reg((i + 1) as u8, arg.clone());
                }

                return Ok(ExecutionResult::Continue);
            } else {
                let mut found = false;
                let mut found_method = None;
                let mut found_iface_name = None;

                for iface_name in &class.parent_interfaces {
                    if let Some(iface) = self.program.classes.get(iface_name) {
                        let method = iface.methods.iter().find(|(k, _)| {
                            if let Some(paren_pos) = k.find('(') {
                                &k[..paren_pos] == method_name
                            } else {
                                k.as_str() == method_name
                            }
                        }).map(|(_, v)| v.clone());

                        if let Some(m) = method {
                            found_method = Some(m);
                            found_iface_name = Some(iface_name.clone());
                            found = true;
                            break;
                        }
                    }
                }

                if found {
                    let method = found_method.unwrap();
                    let iface_name = found_iface_name.unwrap();

                    // Calculate new frame base: start after the caller's register window
                    let caller_frame = self.context.call_stack.last().unwrap();
                    let new_frame_base = caller_frame.frame_base + caller_frame.register_count as usize;

                    if new_frame_base + method.register_count as usize > self.context.registers.len() {
                        return Err(Value::String("Register overflow in interface method call".to_string()));
                    }

                    let new_frame = CallFrame::new(
                        0,
                        new_frame_base,
                        arg_count,
                        method.register_count,
                        format!("{}.{}", iface_name, method_name),
                        self.current_source_file(),
                        Arc::new(method.bytecode.clone()),
                        self.program.strings.clone(),
                        self.program.functions.clone(),
                        self.program.classes.clone(),
                        Some(rd),
                    );

                    self.push_frame(new_frame);

                    for (i, arg) in args.iter().enumerate() {
                        self.set_reg((i + 1) as u8, arg.clone());
                    }

                    return Ok(ExecutionResult::Continue);
                } else {
                    return Err(Value::String(format!("Interface method '{}' not found in class '{}' or its interfaces", method_name, class_name)));
                }
            }
        } else {
            return Err(Value::String(format!("Class '{}' not found", class_name)));
        }
    }

    #[inline]
    fn op_return(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        let return_value = self.get_reg(rs).clone();

        let frame = self.pop_frame().unwrap();
        if let Some(rd) = frame.return_reg {
            self.set_reg(rd, return_value);
        } else {
            self.set_reg(0, return_value);
        }

        return Ok(ExecutionResult::Continue);
    }

    #[inline]
    fn op_jump(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let target = u16::from_le_bytes([
            self.program.bytecode[self.pc()],
            self.program.bytecode[self.pc() + 1],
        ]) as usize;
        self.set_pc(target);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_jump_if_true(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let target = u16::from_le_bytes([
            self.program.bytecode[self.pc()],
            self.program.bytecode[self.pc() + 1],
        ]) as usize;
        if self.get_reg(rs).is_truthy() {
            self.set_pc(target);
        } else {
            self.set_pc(self.pc() + 2);
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_jump_if_false(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let target = u16::from_le_bytes([
            self.program.bytecode[self.pc()],
            self.program.bytecode[self.pc() + 1],
        ]) as usize;
        if !self.get_reg(rs).is_truthy() {
            self.set_pc(target);
        } else {
            self.set_pc(self.pc() + 2);
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_equal(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Bool(self.get_reg(rs1) == self.get_reg(rs2)));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_not_equal(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Bool(self.get_reg(rs1) != self.get_reg(rs2)));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_greater(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Bool(left.to_arithmetic_int().unwrap() > right.to_arithmetic_int().unwrap())
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Bool(left.to_float().unwrap() > right.to_float().unwrap())
            }
            _ => Value::Bool(false),
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_less(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Bool(left.to_arithmetic_int().unwrap() < right.to_arithmetic_int().unwrap())
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Bool(left.to_float().unwrap() < right.to_float().unwrap())
            }
            _ => Value::Bool(false),
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_greater_equal(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Bool(left.to_arithmetic_int().unwrap() >= right.to_arithmetic_int().unwrap())
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Bool(left.to_float().unwrap() >= right.to_float().unwrap())
            }
            _ => Value::Bool(false),
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_less_equal(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Bool(left.to_arithmetic_int().unwrap() <= right.to_arithmetic_int().unwrap())
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Bool(left.to_float().unwrap() <= right.to_float().unwrap())
            }
            _ => Value::Bool(false),
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_and(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Bool(self.get_reg(rs1).is_truthy() && self.get_reg(rs2).is_truthy()));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_or(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Bool(self.get_reg(rs1).is_truthy() || self.get_reg(rs2).is_truthy()));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_not(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        self.set_reg(rd, Value::Bool(!self.get_reg(rs).is_truthy()));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_add(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            (Value::String(a), Value::String(b)) => Value::String(a.clone() + b),
            (Value::String(a), b) => Value::String(a.clone() + &b.to_string()),
            (a, Value::String(b)) => Value::String(a.to_string() + b),
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Int64(left.to_arithmetic_int().unwrap().wrapping_add(right.to_arithmetic_int().unwrap()))
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Float64(left.to_float().unwrap() + right.to_float().unwrap())
            }
            _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                 (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                Value::Float64(left.to_float().unwrap() + right.to_float().unwrap())
            }
            _ => Value::Null,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_subtract(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Int64(left.to_arithmetic_int().unwrap().wrapping_sub(right.to_arithmetic_int().unwrap()))
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Float64(left.to_float().unwrap() - right.to_float().unwrap())
            }
            _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                 (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                Value::Float64(left.to_float().unwrap() - right.to_float().unwrap())
            }
            _ => Value::Null,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_multiply(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                Value::Int64(left.to_arithmetic_int().unwrap().wrapping_mul(right.to_arithmetic_int().unwrap()))
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                Value::Float64(left.to_float().unwrap() * right.to_float().unwrap())
            }
            _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                 (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                Value::Float64(left.to_float().unwrap() * right.to_float().unwrap())
            }
            _ => Value::Null,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_divide(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                let r = right.to_arithmetic_int().unwrap();
                if r != 0 {
                    Value::Int64(left.to_arithmetic_int().unwrap() / r)
                } else {
                    Value::Null
                }
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                let r = right.to_float().unwrap();
                if r != 0.0 {
                    Value::Float64(left.to_float().unwrap() / r)
                } else {
                    Value::Null
                }
            }
            _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                 (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                let r = right.to_float().unwrap();
                if r != 0.0 {
                    Value::Float64(left.to_float().unwrap() / r)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_modulo(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = match (left, right) {
            _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                let r = right.to_arithmetic_int().unwrap();
                if r != 0 {
                    Value::Int64(left.to_arithmetic_int().unwrap() % r)
                } else {
                    Value::Null
                }
            }
            _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                let r = right.to_float().unwrap();
                if r != 0.0 {
                    Value::Float64(left.to_float().unwrap() % r)
                } else {
                    Value::Null
                }
            }
            _ => Value::Null,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_bit_and(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = if left.is_arithmetic_int() && right.is_arithmetic_int() {
            Value::Int64(left.to_arithmetic_int().unwrap() & right.to_arithmetic_int().unwrap())
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_bit_or(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = if left.is_arithmetic_int() && right.is_arithmetic_int() {
            Value::Int64(left.to_arithmetic_int().unwrap() | right.to_arithmetic_int().unwrap())
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_bit_xor(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = if left.is_arithmetic_int() && right.is_arithmetic_int() {
            Value::Int64(left.to_arithmetic_int().unwrap() ^ right.to_arithmetic_int().unwrap())
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_bit_not(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        let value = self.get_reg(rs);
        let result = if value.is_arithmetic_int() {
            Value::Int64(!value.to_arithmetic_int().unwrap())
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_shift_left(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = if left.is_arithmetic_int() && right.is_arithmetic_int() {
            let shift = right.to_arithmetic_int().unwrap() as u32;
            Value::Int64(left.to_arithmetic_int().unwrap().wrapping_shl(shift))
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_shift_right(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs1 = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs2 = self.program.bytecode[self.pc()] as u8;
        let left = self.get_reg(rs1);
        let right = self.get_reg(rs2);
        let result = if left.is_arithmetic_int() && right.is_arithmetic_int() {
            let shift = right.to_arithmetic_int().unwrap() as u32;
            Value::Int64(left.to_arithmetic_int().unwrap().wrapping_shr(shift))
        } else {
            Value::Null
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_concat(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let count = self.program.bytecode[self.pc()] as u8;

        let mut result = String::new();
        for i in 0..count {
            result.push_str(&self.get_reg(rs_start + i).to_string());
        }
        self.set_reg(rd, Value::String(result));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_convert(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let cast_type = self.program.bytecode[self.pc()];

        let value = self.get_reg(rs).clone();
        let result = match cast_type {
            0x01 => {
                match &value {
                    Value::Int64(n) => Value::Int64(*n),
                    Value::Int8(n) => Value::Int64(*n as i64),
                    Value::Int16(n) => Value::Int64(*n as i64),
                    Value::Int32(n) => Value::Int64(*n as i64),
                    Value::UInt8(n) => Value::Int64(*n as i64),
                    Value::UInt16(n) => Value::Int64(*n as i64),
                    Value::UInt32(n) => Value::Int64(*n as i64),
                    Value::UInt64(n) => Value::Int64(*n as i64),
                    Value::Float64(f) => Value::Int64(*f as i64),
                    Value::Float32(f) => Value::Int64(*f as i64),
                    Value::Bool(b) => Value::Int64(if *b { 1 } else { 0 }),
                    Value::String(s) => {
                        if let Ok(n) = s.parse::<i64>() {
                            Value::Int64(n)
                        } else if let Ok(f) = s.parse::<f64>() {
                            Value::Int64(f as i64)
                        } else {
                            Value::Int64(0)
                        }
                    }
                    _ => Value::Int64(0),
                }
            }
            0x02 => {
                match &value {
                    Value::Int64(n) => Value::Float64(*n as f64),
                    Value::Int8(n) => Value::Float64(*n as f64),
                    Value::Int16(n) => Value::Float64(*n as f64),
                    Value::Int32(n) => Value::Float64(*n as f64),
                    Value::UInt8(n) => Value::Float64(*n as f64),
                    Value::UInt16(n) => Value::Float64(*n as f64),
                    Value::UInt32(n) => Value::Float64(*n as f64),
                    Value::UInt64(n) => Value::Float64(*n as f64),
                    Value::Float64(f) => Value::Float64(*f),
                    Value::Float32(f) => Value::Float64(*f as f64),
                    Value::Bool(b) => Value::Float64(if *b { 1.0 } else { 0.0 }),
                    Value::String(s) => {
                        if let Ok(n) = s.parse::<f64>() {
                            Value::Float64(n)
                        } else {
                            Value::Float64(0.0)
                        }
                    }
                    _ => Value::Float64(0.0),
                }
            }
            0x03 => {
                match &value {
                    Value::String(s) => Value::String(s.clone()),
                    _ => Value::String(value.to_string()),
                }
            }
            0x04 => Value::Bool(value.is_truthy()),
            0x05 => Value::Int8(value.to_i8().unwrap_or(0)),
            0x06 => Value::UInt8(value.to_u8().unwrap_or(0)),
            0x07 => Value::Int16(value.to_i16().unwrap_or(0)),
            0x08 => Value::UInt16(value.to_u16().unwrap_or(0)),
            0x09 => Value::Int32(value.to_i32().unwrap_or(0)),
            0x0A => Value::UInt32(value.to_u32().unwrap_or(0)),
            0x0B => Value::Int64(value.to_i64().unwrap_or(0)),
            0x0C => Value::UInt64(value.to_u64().unwrap_or(0)),
            0x0D => Value::Float32(value.to_f32().unwrap_or(0.0)),
            0x0E => Value::Float64(value.to_f64().unwrap_or(0.0)),
            _ => value,
        };
        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_array(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let rs_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let count = self.program.bytecode[self.pc()] as u8;

        let mut elements = Vec::new();
        for i in 0..count {
            elements.push(self.get_reg(rs_start + i).clone());
        }

        self.set_reg(rd, Value::Array(Arc::new(Mutex::new(elements))));
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_index(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rd = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let r_obj = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let r_idx = self.program.bytecode[self.pc()] as u8;

        let obj = self.get_reg(r_obj).clone();
        let idx_val = self.get_reg(r_idx).clone();

        let result = match obj {
            Value::Array(arr) => {
                let idx = idx_val.to_int().unwrap_or(0) as usize;
                let elements = arr.lock().unwrap();
                elements.get(idx).cloned().unwrap_or(Value::Null)
            }
            Value::String(s) => {
                let idx = idx_val.to_int().unwrap_or(0) as usize;
                s.chars().nth(idx).map(|c| Value::String(c.to_string())).unwrap_or(Value::Null)
            }
            _ => Value::Null,
        };

        self.set_reg(rd, result);
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_line(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let line = u16::from_le_bytes([
            self.program.bytecode[self.pc()],
            self.program.bytecode[self.pc() + 1],
        ]) as usize;
        self.set_line(line);
        self.set_pc(self.pc() + 2);

        if self.is_debugging {
            if let Some(ref file) = self.current_source_file() {
                if self.breakpoints.contains(&(file.clone(), line)) {
                    return Ok(ExecutionResult::Breakpoint);
                }
            }
        }
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_try_start(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let catch_pc = u16::from_le_bytes([
            self.program.bytecode[self.pc()],
            self.program.bytecode[self.pc() + 1],
        ]) as usize;
        self.set_pc(self.pc() + 2);
        let catch_reg = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);

        self.context.exception_handlers.push(ExceptionHandler {
            catch_pc,
            catch_register: catch_reg as usize,
            call_stack_depth: self.context.call_stack.len(),
        });
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_try_end(&mut self) -> Result<ExecutionResult, Value> {
        self.context.exception_handlers.pop();
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_throw(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let rs = self.program.bytecode[self.pc()] as u8;
        Err(self.get_reg(rs).clone())
    }

    #[inline]
    fn op_yield(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_spawn(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        let func_idx_lo = self.program.bytecode[self.pc()] as u16;
        self.set_pc(self.pc() + 1);
        let func_idx_hi = self.program.bytecode[self.pc()] as u16;
        self.set_pc(self.pc() + 1);
        let func_index = (func_idx_hi << 8) | func_idx_lo;
        let arg_start = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);
        let arg_count = self.program.bytecode[self.pc()] as u8;
        self.set_pc(self.pc() + 1);

        // Get the function name from the string table
        let func_name = self.program.strings.get(func_index as usize)
            .ok_or_else(|| Value::String(format!("Invalid function index: {}", func_index)))?
            .clone();

        // Collect arguments
        let mut args = Vec::new();
        for i in 0..arg_count {
            args.push(self.get_reg(arg_start + i).clone());
        }

        // Check if we have green thread context
        let spawned_vm = if let Some(ctx) = &self.program.green_thread_ctx {
            let mut vm = VM::new();
            vm.program.native_registry = ctx.native_registry.clone();
            // Share the data structures via Arc (they're immutable)
            vm.program.strings = self.program.strings.clone();
            vm.program.classes = self.program.classes.clone();
            vm.program.functions = self.program.functions.clone();
            vm.program.vtables = ctx.bytecode.vtables.clone();
            
            // Clear any existing call stack
            vm.context.call_stack.clear();

            if vm.call_function(&func_name, args).is_ok() {
                Some(vm)
            } else {
                None
            }
        } else {
            None
        };

        // Store the spawned VM in the context's pending_spawns
        if let Some(vm) = spawned_vm {
            if let Some(ctx) = &self.program.green_thread_ctx {
                ctx.pending_spawns.borrow_mut().push(vm);
            }
        }

        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_breakpoint(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Breakpoint)
    }

    #[inline]
    fn op_halt(&mut self) -> Result<ExecutionResult, Value> {
        self.set_pc(self.pc() + 1);
        Ok(ExecutionResult::Continue)
    }

    #[inline]
    fn op_unknown(&mut self) -> Result<ExecutionResult, Value> {
        let opcode = self.program.bytecode[self.pc()];
        Err(Value::String(format!("Unknown opcode: 0x{:02X}", opcode)))
    }
}

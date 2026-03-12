use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as TokioMutex;
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::ser::SerializeMap;
use serde::de::{MapAccess, Visitor};
use std::fmt;
use async_recursion::async_recursion;
use std::any::Any;
use crate::linker::NativeFunctionRegistry;

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
    Promise(Arc<TokioMutex<PromiseState>>),
    Exception(Exception),
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
            Value::Promise(_) => write!(f, "Promise(...)"),
            Value::Exception(e) => write!(f, "Exception({})", e.message),
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
            (Value::Promise(a), Value::Promise(b)) => Arc::ptr_eq(a, b),
            (Value::Exception(a), Value::Exception(b)) => a.message == b.message,
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
                        Value::Promise(_) => "[promise]".to_string(),
                        Value::Exception(e) => format!("[exception: {}]", e.message),
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
            Value::Promise(_) => "[promise]".to_string(),
            Value::Exception(e) => e.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum PromiseState {
    Pending,
    Resolved(Value),
    Rejected(String),
}

#[derive(Clone)]
pub struct Instance {
    pub class: String,
    pub fields: HashMap<String, Value>,
    pub native_data: Arc<Mutex<Option<Box<dyn Any + Send + Sync>>>>,
}

#[derive(Clone)]
pub struct Class {
    pub name: String,
    pub fields: HashMap<String, Value>,
    pub methods: HashMap<String, Method>,
    pub native_methods: HashMap<String, NativeFn>,
    pub native_create: Option<NativeFn>,
    pub is_native: bool,
    pub parent_interfaces: Vec<String>,
    pub vtable: Vec<String>,  // Ordered list of virtual method names
    pub is_interface: bool,
}

#[derive(Clone)]
pub struct Method {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub register_count: u8,
}

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub bytecode: Vec<u8>,
    pub param_count: u8,
    pub register_count: u8,
    pub source_file: Option<String>,
}

pub type NativeFn = fn(&mut Vec<Value>) -> Result<Value, Value>;

pub struct NativeFunctionBuilder {
    name: String,
    func: NativeFn,
    param_count: Option<usize>,
    return_type: Option<String>,
    description: Option<String>,
}

impl NativeFunctionBuilder {
    pub fn new(name: &str, func: NativeFn) -> Self {
        Self {
            name: name.to_string(),
            func,
            param_count: None,
            return_type: None,
            description: None,
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

    pub fn description(mut self, desc: &str) -> Self {
        self.description = Some(desc.to_string());
        self
    }

    pub fn register(self, vm: &mut VM) {
        vm.register_native(&self.name, self.func);
    }
}

pub struct NativeModule {
    name: String,
    functions: Vec<(String, NativeFn)>,
    class_methods: Vec<(String, String, NativeFn)>,
    class_native_create_callbacks: Vec<(String, NativeFn)>,
}

impl NativeModule {
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            functions: Vec::new(),
            class_methods: Vec::new(),
            class_native_create_callbacks: Vec::new(),
        }
    }

    pub fn function(mut self, name: &str, func: NativeFn) -> Self {
        self.functions.push((name.to_string(), func));
        self
    }

    pub fn class_method(mut self, class_name: &str, method_name: &str, func: NativeFn) -> Self {
        let full_class_name = if class_name.contains('.') {
            class_name.to_string()
        } else {
            format!("{}.{}", self.name, class_name)
        };
        self.class_methods.push((full_class_name, method_name.to_string(), func));
        self
    }

    pub fn class_native_create(mut self, class_name: &str, func: NativeFn) -> Self {
        let full_class_name = if class_name.contains('.') {
            class_name.to_string()
        } else {
            format!("{}.{}", self.name, class_name)
        };
        self.class_native_create_callbacks.push((full_class_name, func));
        self
    }

    pub fn register(self, vm: &mut VM) {
        for (name, func) in self.functions {
            let full_name = format!("{}.{}", self.name, name);
            vm.register_native(&full_name, func);
        }
        for (class_name, method_name, func) in self.class_methods {
            vm.register_native_method(&class_name, &method_name, func);
        }
        for (class_name, func) in self.class_native_create_callbacks {
            vm.register_class_native_create(&class_name, func);
        }
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
}

impl CallFrame {
    pub fn new(
        pc: usize,
        frame_base: usize,
        param_count: u8,
        register_count: u8,
        function_name: String,
        source_file: Option<String>,
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
        }
    }

    pub fn native(
        frame_base: usize,
        param_count: u8,
        function_name: String,
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
        }
    }
}

pub struct VM {
    /// The current bytecode being executed
    bytecode: Vec<u8>,
    /// The register file - fixed size array of values
    /// In a true registry VM, registers are allocated per-frame
    pub registers: Vec<Value>,
    /// String constant pool
    strings: Vec<String>,
    /// Local variables (for module-level code)
    locals: HashMap<String, Value>,
    /// Class definitions
    classes: HashMap<String, Class>,
    /// Function definitions
    functions: HashMap<String, Function>,
    /// Native function registry with indexed lookup (optimized)
    pub native_registry: NativeFunctionRegistry,
    /// Legacy native functions HashMap (for backward compatibility)
    #[deprecated(since = "0.2.0", note = "Use native_registry instead")]
    pub native_functions: HashMap<String, NativeFn>,
    /// Fallback native handler
    pub fallback_native: Option<NativeFn>,
    /// Pending native methods to be attached to classes
    pending_native_methods: HashMap<String, HashMap<String, NativeFn>>,
    /// Pending class native_create callbacks
    pending_class_native_create: HashMap<String, NativeFn>,
    /// Exception handlers
    exception_handlers: Vec<ExceptionHandler>,
    /// Call stack - frames for active function calls
    call_stack: Vec<CallFrame>,
    /// Source file for the current execution context
    source_file: Option<String>,
    /// Current line being executed
    current_line: usize,
    /// Breakpoints for debugging
    pub breakpoints: std::collections::HashSet<(String, usize)>,
    /// Whether debugging mode is enabled
    pub is_debugging: bool,
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
            bytecode: Vec::new(),
            // 256 registers - typical for registry-based VMs
            // Each frame gets a window of registers
            registers: vec![Value::Null; 256],
            strings: Vec::new(),
            locals: HashMap::new(),
            classes: HashMap::new(),
            functions: HashMap::new(),
            native_registry: NativeFunctionRegistry::new(),
            #[allow(deprecated)]
            native_functions: HashMap::new(),
            fallback_native: None,
            pending_native_methods: HashMap::new(),
            pending_class_native_create: HashMap::new(),
            exception_handlers: Vec::new(),
            call_stack: Vec::new(),
            source_file: None,
            current_line: 1,
            breakpoints: std::collections::HashSet::new(),
            is_debugging: false,
        }
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        // Check if already registered
        if self.native_registry.get_index(name).is_some() {
            // Update existing registration (hot-swap)
            self.native_registry.hot_swap(name, f);
        } else {
            // New registration
            self.native_registry.register(name, f);
        }
        #[allow(deprecated)]
        self.native_functions.insert(name.to_string(), f);
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

    pub fn register_fallback(&mut self, f: NativeFn) {
        self.fallback_native = Some(f);
        self.native_registry.set_fallback(f);
    }

    /// Load bytecode and initialize the VM
    pub fn load(&mut self, bytecode: &[u8], strings: Vec<String>, classes: Vec<Class>, functions: Vec<Function>) -> Result<(), String> {
        self.bytecode = bytecode.to_vec();
        self.strings = strings;
        
        self.classes.clear();
        for mut class in classes {
            if let Some(methods) = self.pending_native_methods.get(&class.name) {
                for (method_name, func) in methods {
                    class.native_methods.insert(method_name.clone(), *func);
                }
            }
            if let Some(on_init) = self.pending_class_native_create.get(&class.name) {
                class.native_create = Some(*on_init);
            }
            self.classes.insert(class.name.clone(), class);
        }
        
        self.functions.clear();
        for function in functions {
            self.functions.insert(function.name.clone(), function);
        }

        // Initialize for execution
        self.set_pc(0);
        self.registers.fill(Value::Null);
        
        // Create initial call frame for module-level code
        self.call_stack = vec![CallFrame::new(
            0,
            0,
            0,
            16,
            "<main>".to_string(),
            self.source_file.clone(),
        )];
        
        self.current_line = 1;
        Ok(())
    }

    /// Get current PC from the top frame
    #[inline]
    fn pc(&self) -> usize {
        self.call_stack.last().map(|f| f.pc).unwrap_or(0)
    }

    /// Set current PC in the top frame
    #[inline]
    fn set_pc(&mut self, pc: usize) {
        if let Some(frame) = self.call_stack.last_mut() {
            frame.pc = pc;
        }
    }

    /// Get current frame base
    #[inline]
    fn frame_base(&self) -> usize {
        self.call_stack.last().map(|f| f.frame_base).unwrap_or(0)
    }

    /// Read a register relative to the current frame base
    #[inline]
    fn get_reg(&self, offset: u8) -> &Value {
        let idx = self.frame_base() + offset as usize;
        &self.registers[idx]
    }

    /// Write to a register relative to the current frame base
    #[inline]
    fn set_reg(&mut self, offset: u8, value: Value) {
        let idx = self.frame_base() + offset as usize;
        self.registers[idx] = value;
    }

    /// Clone a register value
    #[inline]
    fn clone_reg(&mut self, dest: u8, src: u8) {
        let value = self.get_reg(src).clone();
        self.set_reg(dest, value);
    }

    pub fn set_source_file(&mut self, file: &str) {
        self.source_file = Some(file.to_string());
    }

    pub fn set_line(&mut self, line: usize) {
        self.current_line = line;
        if let Some(frame) = self.call_stack.last_mut() {
            frame.line_number = line;
        }
    }

    pub fn get_line(&self) -> usize {
        self.current_line
    }

    pub fn get_source_file(&self) -> Option<String> {
        self.source_file.clone()
    }

    #[async_recursion]
    pub async fn run(&mut self) -> Result<RunResult, Value> {
        while self.pc() < self.bytecode.len() {
            let opcode = self.bytecode[self.pc()];
            let result = match self.execute(opcode).await {
                Ok(res) => res,
                Err(e) => {
                    let exception = match &e {
                        Value::Exception(existing) => existing.clone(),
                        _ => self.build_exception(&e),
                    };

                    // Only catch if we have a handler in the current call frame
                    let mut has_local_handler = false;
                    if let Some(handler) = self.exception_handlers.last() {
                        if handler.call_stack_depth == self.call_stack.len() {
                            has_local_handler = true;
                        }
                    }

                    if has_local_handler {
                        let handler = self.exception_handlers.pop().unwrap();
                        self.set_pc(handler.catch_pc);
                        self.set_reg(handler.catch_register as u8, Value::Exception(exception));
                        continue;
                    } else {
                        return Err(Value::Exception(exception));
                    }
                }
            };

            if opcode == Opcode::Halt as u8 || opcode == Opcode::Return as u8 {
                break;
            }

            match result {
                ExecutionResult::Awaiting(promise) => return Ok(RunResult::Awaiting(promise)),
                ExecutionResult::Breakpoint => {
                    return Ok(RunResult::Breakpoint);
                }
                ExecutionResult::Continue => {}
            }

            // execute() is responsible for setting PC to next instruction
            // No increment needed here
        }

        Ok(RunResult::Finished(Some(self.get_reg(0).clone())))
    }

    fn build_exception(&self, value: &Value) -> Exception {
        let message = match value {
            Value::String(s) => s.clone(),
            Value::Exception(e) => e.message.clone(),
            _ => value.to_string(),
        };

        let stack_trace: Vec<StackFrame> = self.call_stack.iter().map(|frame| {
            StackFrame {
                function_name: frame.function_name.clone(),
                source_file: frame.source_file.clone(),
                line_number: Some(frame.line_number),
            }
        }).collect();

        Exception::new(message, stack_trace)
    }

    #[async_recursion]
    async fn execute(&mut self, opcode: u8) -> Result<ExecutionResult, Value> {
        match opcode {
            x if x == Opcode::Nop as u8 => {
                self.set_pc(self.pc() + 1);
            }

            // Load constant string into register
            // Format: [LoadConst, Rd, string_idx]
            x if x == Opcode::LoadConst as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let idx = self.bytecode[self.pc()] as usize;
                let s = self.strings.get(idx)
                    .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
                    .clone();
                self.set_reg(rd, Value::String(s));
                self.set_pc(self.pc() + 1);
            }

            // Load 64-bit integer into register
            // Format: [LoadInt, Rd, 8 bytes (little endian)]
            x if x == Opcode::LoadInt as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let bytes: [u8; 8] = self.bytecode[self.pc()..self.pc() + 8]
                    .try_into()
                    .map_err(|_| Value::String("Invalid int encoding".to_string()))?;
                let n = i64::from_le_bytes(bytes);
                self.set_reg(rd, Value::Int64(n));
                self.set_pc(self.pc() + 8);
            }

            // Load 64-bit float into register
            // Format: [LoadFloat, Rd, 8 bytes (little endian)]
            x if x == Opcode::LoadFloat as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let bytes: [u8; 8] = self.bytecode[self.pc()..self.pc() + 8]
                    .try_into()
                    .map_err(|_| Value::String("Invalid float encoding".to_string()))?;
                let n = f64::from_le_bytes(bytes);
                self.set_reg(rd, Value::Float64(n));
                self.set_pc(self.pc() + 8);
            }

            // Load boolean into register
            // Format: [LoadBool, Rd, 0/1]
            x if x == Opcode::LoadBool as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let b = self.bytecode[self.pc()] != 0;
                self.set_reg(rd, Value::Bool(b));
                self.set_pc(self.pc() + 1);
            }

            // Load null into register
            // Format: [LoadNull, Rd]
            x if x == Opcode::LoadNull as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_reg(rd, Value::Null);
                self.set_pc(self.pc() + 1);
            }

            // Move value from one register to another
            // Format: [Move, Rd, Rs]
            x if x == Opcode::Move as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.clone_reg(rd, rs);
                self.set_pc(self.pc() + 1);
            }

            // Load local variable into register
            // Format: [LoadLocal, Rd, name_idx]
            x if x == Opcode::LoadLocal as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let idx = self.bytecode[self.pc()] as usize;
                let name = self.strings.get(idx)
                    .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
                    .clone();
                let value = self.locals.get(&name).cloned().unwrap_or(Value::Null);
                self.set_reg(rd, value);
                self.set_pc(self.pc() + 1);
            }

            // Store register value to local variable
            // Format: [StoreLocal, name_idx, Rs]
            x if x == Opcode::StoreLocal as u8 => {
                self.set_pc(self.pc() + 1);
                let idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                let name = self.strings.get(idx)
                    .ok_or_else(|| Value::String(format!("Invalid string index: {}", idx)))?
                    .clone();
                let value = self.get_reg(rs).clone();
                self.locals.insert(name, value);
                self.set_pc(self.pc() + 1);
            }

            // Get property from instance
            // Format: [GetProperty, Rd, Robj, name_idx]
            x if x == Opcode::GetProperty as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let robj = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let idx = self.bytecode[self.pc()] as usize;
                let name = self.strings.get(idx)
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
                    _ => {
                        return Err(Value::String(format!("Expected instance for property get, got {:?}", robj_val)));
                    }
                }
                self.set_pc(self.pc() + 1);
            }

            // Set property on instance
            // Format: [SetProperty, Robj, name_idx, Rs]
            x if x == Opcode::SetProperty as u8 => {
                self.set_pc(self.pc() + 1);
                let robj = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                let name = self.strings.get(idx)
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
            }

            // Call function
            // Format: [Call, Rd, func_idx, arg_start, arg_count]
            // Rd receives the result, args are in registers [arg_start..arg_start+arg_count]
            x if x == Opcode::Call as u8 || x == Opcode::CallAsync as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let func_idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let arg_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let arg_count = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);  // Advance PC past all operands

                let func_name = self.strings.get(func_idx)
                    .ok_or_else(|| Value::String(format!("Invalid function index: {}", func_idx)))?
                    .clone();

                // Check if it's a class constructor
                if let Some(class) = self.classes.get(&func_name).cloned() {
                    let instance = Value::Instance(Arc::new(Mutex::new(Instance {
                        class: func_name.clone(),
                        fields: class.fields.clone(),
                        native_data: Arc::new(Mutex::new(None)),
                    })));
                    self.set_reg(rd, instance.clone());

                    // Call native_create if it exists
                    if let Some(native_create) = class.native_create {
                        let mut args = vec![instance];
                        native_create(&mut args)?;
                    }
                } 
                // Check if it's a native function
                else if let Some(native_f) = self.native_functions.get(&func_name) {
                    let mut args = Vec::new();
                    for i in 0..arg_count {
                        args.push(self.get_reg(arg_start + i).clone());
                    }
                    let result = native_f(&mut args)?;
                    self.set_reg(rd, result);
                } 
                // Check if it's a bytecode function
                else if let Some(function) = self.functions.get(&func_name).cloned() {
                    // Collect arguments
                    let mut args = Vec::new();
                    for i in 0..arg_count {
                        args.push(self.get_reg(arg_start + i).clone());
                    }

                    // Save caller's state
                    let caller_pc = self.pc();
                    let caller_frame_base = self.frame_base();
                    let caller_source = self.source_file.clone();
                    let caller_line = self.current_line;

                    // Calculate new frame base - place args in R1..Rn of new frame
                    // R0 of new frame will be the return value
                    let new_frame_base = caller_frame_base + arg_start as usize + arg_count as usize;
                    
                    // Check register bounds
                    if new_frame_base + function.register_count as usize > self.registers.len() {
                        return Err(Value::String("Register overflow: too many nested calls".to_string()));
                    }

                    // Set up new frame
                    let mut new_call_stack = self.call_stack.clone();
                    if let Some(last_frame) = new_call_stack.last_mut() {
                        last_frame.source_file = caller_source.clone();
                        last_frame.line_number = caller_line;
                    }
                    new_call_stack.push(CallFrame::new(
                        0,
                        new_frame_base,
                        function.param_count,
                        function.register_count,
                        func_name.clone(),
                        function.source_file.clone(),
                    ));
                    self.call_stack = new_call_stack;

                    // Copy arguments to parameter registers (R1..Rn)
                    for (i, arg) in args.iter().enumerate() {
                        self.set_reg((i + 1) as u8, arg.clone());
                    }

                    // Load function bytecode - clone data first to avoid borrow issues
                    let new_bytecode = function.bytecode.clone();
                    let new_strings = self.strings.clone();
                    let new_functions = self.functions.clone();
                    let new_native_functions = self.native_functions.clone();
                    let new_classes = self.classes.clone();

                    let old_bytecode = std::mem::replace(&mut self.bytecode, new_bytecode);
                    let old_strings = std::mem::replace(&mut self.strings, new_strings);
                    let old_functions = std::mem::replace(&mut self.functions, new_functions);
                    let old_native_functions = std::mem::replace(&mut self.native_functions, new_native_functions);
                    let old_classes = std::mem::replace(&mut self.classes, new_classes);

                    // Execute function
                    let result = self.run().await;

                    // Restore state
                    self.bytecode = old_bytecode;
                    self.strings = old_strings;
                    self.functions = old_functions;
                    self.native_functions = old_native_functions;
                    self.classes = old_classes;

                    // Pop frame and restore caller
                    self.call_stack.pop();
                    if let Some(frame) = self.call_stack.last_mut() {
                        frame.pc = caller_pc;  // PC already points to next instruction
                        frame.frame_base = caller_frame_base;
                    }

                    match result {
                        Ok(RunResult::Finished(val)) => self.set_reg(rd, val.unwrap_or(Value::Null)),
                        Ok(RunResult::Breakpoint) => return Ok(ExecutionResult::Breakpoint),
                        Ok(RunResult::Awaiting(promise)) => return Ok(ExecutionResult::Awaiting(promise)),
                        Err(e) => return Err(e),
                    }
                } else {
                    return Err(Value::String(format!("Function not found: {}", func_name)));
                }
            }

            // Call native function
            // Format: [CallNative, Rd, name_idx, arg_start, arg_count]
            x if x == Opcode::CallNative as u8 || x == Opcode::CallNativeAsync as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let name_idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let arg_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let arg_count = self.bytecode[self.pc()] as u8;

                let name = self.strings.get(name_idx)
                    .ok_or_else(|| Value::String(format!("Invalid native name index: {}", name_idx)))?
                    .clone();

                let mut args = Vec::new();
                for i in 0..arg_count {
                    args.push(self.get_reg(arg_start + i).clone());
                }

                // Try indexed lookup first (faster), fall back to HashMap
                let result = match self.native_registry.get_index(&name).and_then(|idx| self.native_registry.get_by_index(idx)) {
                    Some(f) => f(&mut args)?,
                    None => {
                        // Fall back to legacy HashMap lookup
                        #[allow(deprecated)]
                        match self.native_functions.get(&name) {
                            Some(f) => f(&mut args)?,
                            None => {
                                match &self.fallback_native {
                                    Some(f) => f(&mut args)?,
                                    None => {
                                        return Err(Value::String(format!("Native function not found: {}", name)));
                                    }
                                }
                            }
                        }
                    }
                };
                self.set_reg(rd, result);
                self.set_pc(self.pc() + 1);
            }
            
            // Call native function by index (optimized - O(1) lookup)
            // Format: [CallNativeIndexed, Rd, func_idx_lo, func_idx_hi, arg_start, arg_count]
            x if x == Opcode::CallNativeIndexed as u8 || x == Opcode::CallNativeIndexedAsync as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                // Read u16 function index (little-endian)
                let func_idx_lo = self.bytecode[self.pc()] as u16;
                self.set_pc(self.pc() + 1);
                let func_idx_hi = self.bytecode[self.pc()] as u16;
                self.set_pc(self.pc() + 1);
                let func_index = (func_idx_hi << 8) | func_idx_lo;
                
                let arg_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let arg_count = self.bytecode[self.pc()] as u8;

                let mut args = Vec::new();
                for i in 0..arg_count {
                    args.push(self.get_reg(arg_start + i).clone());
                }

                // Direct indexed lookup - O(1)
                let result = match self.native_registry.get_by_index(func_index) {
                    Some(f) => f(&mut args)?,
                    None => {
                        match &self.fallback_native {
                            Some(f) => f(&mut args)?,
                            None => {
                                return Err(Value::String(format!("Native function not found at index: {}", func_index)));
                            }
                        }
                    }
                };
                self.set_reg(rd, result);
                self.set_pc(self.pc() + 1);
            }

            // Invoke method on instance
            // Format: [Invoke, Rd, method_idx, arg_start, arg_count]
            // First argument (arg_start) is the receiver (self)
            x if x == Opcode::Invoke as u8 || x == Opcode::InvokeAsync as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let method_idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let arg_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let arg_count = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);  // Advance PC past all operands

                let name = self.strings.get(method_idx)
                    .ok_or_else(|| Value::String(format!("Invalid method index: {}", method_idx)))?
                    .clone();

                let mut args = Vec::new();
                for i in 0..arg_count {
                    args.push(self.get_reg(arg_start + i).clone());
                }

                // Check if this is an array method call
                if let Some(Value::Array(_)) = args.first() {
                    // Handle array native methods directly
                    let result = match name.as_str() {
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
                } else {
                    let instance = if let Some(Value::Instance(instance)) = args.first() {
                        instance.clone()
                    } else {
                        return Err(Value::String("Invoke requires an instance".to_string()));
                    };

                    let class_name = instance.lock().unwrap().class.clone();
                    if let Some(class) = self.classes.get(&class_name).cloned() {
                        if let Some(native_method) = class.native_methods.get(&name) {
                            let mut method_args = args.clone();
                            let result = native_method(&mut method_args)?;
                            self.set_reg(rd, result);
                        } else if let Some(method) = class.methods.get(&name) {
                            // Set up method call frame
                            let caller_pc = self.pc();
                            let caller_frame_base = self.frame_base();

                            let new_frame_base = caller_frame_base + arg_start as usize + arg_count as usize;

                            if new_frame_base + method.register_count as usize > self.registers.len() {
                                return Err(Value::String("Register overflow in method call".to_string()));
                            }

                            let mut new_call_stack = self.call_stack.clone();
                            if let Some(last_frame) = new_call_stack.last_mut() {
                                last_frame.line_number = self.current_line;
                            }
                            new_call_stack.push(CallFrame::new(
                                0,
                                new_frame_base,
                                arg_count,
                                method.register_count,
                                format!("{}.{}", class_name, name),
                                self.source_file.clone(),
                            ));
                            self.call_stack = new_call_stack;

                            // Copy arguments (first is self, placed in R1..Rn)
                            for (i, arg) in args.iter().enumerate() {
                                self.set_reg((i + 1) as u8, arg.clone());
                            }

                            let new_bytecode = method.bytecode.clone();
                            let new_strings = self.strings.clone();
                            let new_classes = self.classes.clone();
                            let new_native_functions = self.native_functions.clone();

                            let old_bytecode = std::mem::replace(&mut self.bytecode, new_bytecode);
                            let old_strings = std::mem::replace(&mut self.strings, new_strings);
                            let old_classes = std::mem::replace(&mut self.classes, new_classes);
                            let old_native_functions = std::mem::replace(&mut self.native_functions, new_native_functions);

                            let result = self.run().await;

                            self.bytecode = old_bytecode;
                            self.strings = old_strings;
                            self.classes = old_classes;
                            self.native_functions = old_native_functions;

                            self.call_stack.pop();
                            if let Some(frame) = self.call_stack.last_mut() {
                                frame.pc = caller_pc;  // PC already points to next instruction
                                frame.frame_base = caller_frame_base;
                            }

                            match result {
                                Ok(RunResult::Finished(val)) => self.set_reg(rd, val.unwrap_or(Value::Null)),
                                Ok(RunResult::Breakpoint) => return Ok(ExecutionResult::Breakpoint),
                                Ok(RunResult::Awaiting(promise)) => return Ok(ExecutionResult::Awaiting(promise)),
                                Err(e) => return Err(e),
                            }
                        } else {
                            return Err(Value::String(format!("Method '{}' not found on class '{}'", name, class_name)));
                        }
                    } else {
                        return Err(Value::String(format!("Class '{}' not found", class_name)));
                    }
                }
            }

            // Invoke interface method via vtable
            // Format: [InvokeInterface, Rd, vtable_idx, arg_start, arg_count]
            // First argument (arg_start) is the receiver (self)
            x if x == Opcode::InvokeInterface as u8 || x == Opcode::InvokeInterfaceAsync as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let method_idx = self.bytecode[self.pc()] as usize;
                self.set_pc(self.pc() + 1);
                let arg_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let arg_count = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);  // Advance PC past all operands

                let name = self.strings.get(method_idx)
                    .ok_or_else(|| Value::String(format!("Invalid method index: {}", method_idx)))?
                    .clone();

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
                
                // Look up the method through the vtable
                if let Some(class) = self.classes.get(&class_name).cloned() {
                    // First check if the class itself has the method
                    if let Some(method) = class.methods.get(&name) {
                        // Found the method in the class, invoke it
                        let caller_pc = self.pc();
                        let caller_frame_base = self.frame_base();

                        let new_frame_base = caller_frame_base + arg_start as usize + arg_count as usize;

                        if new_frame_base + method.register_count as usize > self.registers.len() {
                            return Err(Value::String("Register overflow in interface method call".to_string()));
                        }

                        let mut new_call_stack = self.call_stack.clone();
                        if let Some(last_frame) = new_call_stack.last_mut() {
                            last_frame.line_number = self.current_line;
                        }
                        new_call_stack.push(CallFrame::new(
                            0,
                            new_frame_base,
                            arg_count,
                            method.register_count,
                            format!("{}.{}", class_name, name),
                            self.source_file.clone(),
                        ));
                        self.call_stack = new_call_stack;

                        for (i, arg) in args.iter().enumerate() {
                            self.set_reg((i + 1) as u8, arg.clone());
                        }

                        let new_bytecode = method.bytecode.clone();
                        let new_strings = self.strings.clone();
                        let new_classes = self.classes.clone();
                        let new_native_functions = self.native_functions.clone();

                        let old_bytecode = std::mem::replace(&mut self.bytecode, new_bytecode);
                        let old_strings = std::mem::replace(&mut self.strings, new_strings);
                        let old_classes = std::mem::replace(&mut self.classes, new_classes);
                        let old_native_functions = std::mem::replace(&mut self.native_functions, new_native_functions);

                        let result = self.run().await;

                        self.bytecode = old_bytecode;
                        self.strings = old_strings;
                        self.classes = old_classes;
                        self.native_functions = old_native_functions;

                        self.call_stack.pop();
                        if let Some(frame) = self.call_stack.last_mut() {
                            frame.pc = caller_pc;
                            frame.frame_base = caller_frame_base;
                        }

                        match result {
                            Ok(RunResult::Finished(val)) => self.set_reg(rd, val.unwrap_or(Value::Null)),
                            Ok(RunResult::Breakpoint) => return Ok(ExecutionResult::Breakpoint),
                            Ok(RunResult::Awaiting(promise)) => return Ok(ExecutionResult::Awaiting(promise)),
                            Err(e) => return Err(e),
                        }
                    } else {
                        // Check parent interfaces for the method (default implementation)
                        let mut found = false;
                        let mut found_method = None;
                        let mut found_iface_name = None;
                        
                        for iface_name in &class.parent_interfaces {
                            if let Some(iface) = self.classes.get(iface_name) {
                                if let Some(method) = iface.methods.get(&name) {
                                    found_method = Some(method.clone());
                                    found_iface_name = Some(iface_name.clone());
                                    found = true;
                                    break;
                                }
                            }
                        }
                        
                        if found {
                            let method = found_method.unwrap();
                            let iface_name = found_iface_name.unwrap();
                            
                            // Found in parent interface, use default implementation
                            let caller_pc = self.pc();
                            let caller_frame_base = self.frame_base();
                            let new_frame_base = caller_frame_base + arg_start as usize + arg_count as usize;

                            if new_frame_base + method.register_count as usize > self.registers.len() {
                                return Err(Value::String("Register overflow in interface method call".to_string()));
                            }

                            let mut new_call_stack = self.call_stack.clone();
                            if let Some(last_frame) = new_call_stack.last_mut() {
                                last_frame.line_number = self.current_line;
                            }
                            new_call_stack.push(CallFrame::new(
                                0,
                                new_frame_base,
                                arg_count,
                                method.register_count,
                                format!("{}.{}", iface_name, name),
                                self.source_file.clone(),
                            ));
                            self.call_stack = new_call_stack;

                            for (i, arg) in args.iter().enumerate() {
                                self.set_reg((i + 1) as u8, arg.clone());
                            }

                            let new_bytecode = method.bytecode.clone();
                            let new_strings = self.strings.clone();
                            let new_classes = self.classes.clone();
                            let new_native_functions = self.native_functions.clone();

                            let old_bytecode = std::mem::replace(&mut self.bytecode, new_bytecode);
                            let old_strings = std::mem::replace(&mut self.strings, new_strings);
                            let old_classes = std::mem::replace(&mut self.classes, new_classes);
                            let old_native_functions = std::mem::replace(&mut self.native_functions, new_native_functions);

                            let result = self.run().await;

                            self.bytecode = old_bytecode;
                            self.strings = old_strings;
                            self.classes = old_classes;
                            self.native_functions = old_native_functions;

                            self.call_stack.pop();
                            if let Some(frame) = self.call_stack.last_mut() {
                                frame.pc = caller_pc;
                                frame.frame_base = caller_frame_base;
                            }

                            match result {
                                Ok(RunResult::Finished(val)) => self.set_reg(rd, val.unwrap_or(Value::Null)),
                                Ok(RunResult::Breakpoint) => return Ok(ExecutionResult::Breakpoint),
                                Ok(RunResult::Awaiting(promise)) => return Ok(ExecutionResult::Awaiting(promise)),
                                Err(e) => return Err(e),
                            }
                        } else {
                            return Err(Value::String(format!("Interface method '{}' not found in class '{}' or its interfaces", name, class_name)));
                        }
                    }
                } else {
                    return Err(Value::String(format!("Class '{}' not found", class_name)));
                }
            }

            // Await a promise
            // Format: [Await, Rd, Rs]
            x if x == Opcode::Await as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;

                let value = self.get_reg(rs).clone();
                match value {
                    Value::Promise(promise) => {
                        loop {
                            let mut state = promise.lock().await;
                            match &mut *state {
                                PromiseState::Pending => {
                                    drop(state);
                                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                                }
                                PromiseState::Resolved(v) => {
                                    self.set_reg(rd, v.clone());
                                    break;
                                }
                                PromiseState::Rejected(e) => {
                                    return Err(Value::String(format!("Promise rejected: {}", e)));
                                }
                            }
                        }
                    }
                    _ => {
                        return Err(Value::String("Can only await Promise values".to_string()));
                    }
                }
                self.set_pc(self.pc() + 1);
            }

            // Return from function
            // Format: [Return, Rs]
            // Rs value goes into caller's Rd (typically R0 of caller receives result)
            x if x == Opcode::Return as u8 => {
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.set_reg(0, self.get_reg(rs).clone());
                return Ok(ExecutionResult::Continue);
            }

            // Unconditional jump
            // Format: [Jump, target (2 bytes little endian)]
            x if x == Opcode::Jump as u8 => {
                self.set_pc(self.pc() + 1);
                let target = u16::from_le_bytes([
                    self.bytecode[self.pc()],
                    self.bytecode[self.pc() + 1],
                ]) as usize;
                self.set_pc(target);
            }

            // Jump if register is truthy
            // Format: [JumpIfTrue, Rs, target (2 bytes)]
            x if x == Opcode::JumpIfTrue as u8 => {
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let target = u16::from_le_bytes([
                    self.bytecode[self.pc()],
                    self.bytecode[self.pc() + 1],
                ]) as usize;
                if self.get_reg(rs).is_truthy() {
                    self.set_pc(target);
                } else {
                    self.set_pc(self.pc() + 2);
                }
            }

            // Jump if register is falsy
            // Format: [JumpIfFalse, Rs, target (2 bytes)]
            x if x == Opcode::JumpIfFalse as u8 => {
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let target = u16::from_le_bytes([
                    self.bytecode[self.pc()],
                    self.bytecode[self.pc() + 1],
                ]) as usize;
                if !self.get_reg(rs).is_truthy() {
                    self.set_pc(target);
                } else {
                    self.set_pc(self.pc() + 2);
                }
            }

            // Compare equality
            // Format: [Equal, Rd, Rs1, Rs2]
            x if x == Opcode::Equal as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                self.set_reg(rd, Value::Bool(self.get_reg(rs1) == self.get_reg(rs2)));
                self.set_pc(self.pc() + 1);
            }

            // Logical NOT
            // Format: [Not, Rd, Rs]
            x if x == Opcode::Not as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.set_reg(rd, Value::Bool(!self.get_reg(rs).is_truthy()));
                self.set_pc(self.pc() + 1);
            }

            // Type cast
            // Format: [Cast, Rd, Rs, type_code]
            x if x == Opcode::Cast as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let cast_type = self.bytecode[self.pc()];

                let value = self.get_reg(rs).clone();
                let result = match cast_type {
                    0x01 => { // Cast to int
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
                    0x02 => { // Cast to float
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
                    0x03 => { // Cast to str
                        Value::String(value.to_string())
                    }
                    0x04 => { // Cast to bool
                        Value::Bool(value.is_truthy())
                    }
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
            }

            // Create array
            // Format: [Array, Rd, rs_start, count]
            x if x == Opcode::Array as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let count = self.bytecode[self.pc()] as u8;
                
                let mut elements = Vec::new();
                for i in 0..count {
                    elements.push(self.get_reg(rs_start + i).clone());
                }
                
                self.set_reg(rd, Value::Array(Arc::new(Mutex::new(elements))));
                self.set_pc(self.pc() + 1);
            }

            // Index array or object
            // Format: [Index, Rd, Robj, Ridx]
            x if x == Opcode::Index as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let r_obj = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let r_idx = self.bytecode[self.pc()] as u8;
                
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
            }

            // Logical AND
            // Format: [And, Rd, Rs1, Rs2]
            x if x == Opcode::And as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                self.set_reg(rd, Value::Bool(self.get_reg(rs1).is_truthy() && self.get_reg(rs2).is_truthy()));
                self.set_pc(self.pc() + 1);
            }

            // Logical OR
            // Format: [Or, Rd, Rs1, Rs2]
            x if x == Opcode::Or as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                self.set_reg(rd, Value::Bool(self.get_reg(rs1).is_truthy() || self.get_reg(rs2).is_truthy()));
                self.set_pc(self.pc() + 1);
            }

            // Greater than comparison
            // Format: [Greater, Rd, Rs1, Rs2]
            x if x == Opcode::Greater as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // Less than comparison
            // Format: [Less, Rd, Rs1, Rs2]
            x if x == Opcode::Less as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // Greater than or equal comparison
            x if x == Opcode::GreaterEqual as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // Less than or equal comparison
            x if x == Opcode::LessEqual as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // Addition
            // Format: [Add, Rd, Rs1, Rs2]
            x if x == Opcode::Add as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                let left = self.get_reg(rs1);
                let right = self.get_reg(rs2);
                let result = match (left, right) {
                    (Value::String(a), Value::String(b)) => Value::String(a.clone() + b),
                    (Value::String(a), b) => Value::String(a.clone() + &b.to_string()),
                    (a, Value::String(b)) => Value::String(a.to_string() + b),
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() + right.to_arithmetic_int().unwrap())
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
            }

            // Subtraction
            // Format: [Subtract, Rd, Rs1, Rs2]
            x if x == Opcode::Subtract as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                let left = self.get_reg(rs1);
                let right = self.get_reg(rs2);
                let result = match (left, right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() - right.to_arithmetic_int().unwrap())
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
            }

            // Multiplication
            // Format: [Multiply, Rd, Rs1, Rs2]
            x if x == Opcode::Multiply as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
                let left = self.get_reg(rs1);
                let right = self.get_reg(rs2);
                let result = match (left, right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() * right.to_arithmetic_int().unwrap())
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
            }

            // Division
            // Format: [Divide, Rd, Rs1, Rs2]
            x if x == Opcode::Divide as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // Modulo
            // Format: [Modulo, Rd, Rs1, Rs2]
            x if x == Opcode::Modulo as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs1 = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs2 = self.bytecode[self.pc()] as u8;
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
            }

            // String concatenation
            // Format: [Concat, Rd, rs_start, count]
            x if x == Opcode::Concat as u8 => {
                self.set_pc(self.pc() + 1);
                let rd = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let rs_start = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);
                let count = self.bytecode[self.pc()] as u8;

                let mut result = String::new();
                for i in 0..count {
                    result.push_str(&self.get_reg(rs_start + i).to_string());
                }
                self.set_reg(rd, Value::String(result));
                self.set_pc(self.pc() + 1);
            }

            // Line number for debugging
            // Format: [Line, line_number (2 bytes)]
            x if x == Opcode::Line as u8 => {
                self.set_pc(self.pc() + 1);
                let line = u16::from_le_bytes([
                    self.bytecode[self.pc()],
                    self.bytecode[self.pc() + 1],
                ]) as usize;
                self.set_line(line);
                self.set_pc(self.pc() + 2);

                // Check for breakpoint
                if self.is_debugging {
                    if let Some(ref file) = self.source_file {
                        if self.breakpoints.contains(&(file.clone(), line)) {
                            return Ok(ExecutionResult::Breakpoint);
                        }
                    }
                }
            }

            // Exception handling start
            // Format: [TryStart, catch_pc (2 bytes), catch_reg]
            x if x == Opcode::TryStart as u8 => {
                self.set_pc(self.pc() + 1);
                let catch_pc = u16::from_le_bytes([
                    self.bytecode[self.pc()],
                    self.bytecode[self.pc() + 1],
                ]) as usize;
                self.set_pc(self.pc() + 2);
                let catch_reg = self.bytecode[self.pc()] as u8;
                self.set_pc(self.pc() + 1);

                self.exception_handlers.push(ExceptionHandler {
                    catch_pc,
                    catch_register: catch_reg as usize,
                    call_stack_depth: self.call_stack.len(),
                });
            }

            // Exception handling end
            x if x == Opcode::TryEnd as u8 => {
                self.exception_handlers.pop();
                self.set_pc(self.pc() + 1);
            }

            // Throw exception
            // Format: [Throw, Rs]
            x if x == Opcode::Throw as u8 => {
                self.set_pc(self.pc() + 1);
                let rs = self.bytecode[self.pc()] as u8;
                return Err(self.get_reg(rs).clone());
            }

            // Breakpoint for debugging
            x if x == Opcode::Breakpoint as u8 => {
                self.set_pc(self.pc() + 1);
                return Ok(ExecutionResult::Breakpoint);
            }

            // Halt execution
            x if x == Opcode::Halt as u8 => {
                self.set_pc(self.pc() + 1);
                return Ok(ExecutionResult::Continue);
            }

            _ => {
                return Err(Value::String(format!("Unknown opcode: 0x{:02X}", opcode)));
            }
        }

        Ok(ExecutionResult::Continue)
    }
}

#[derive(Debug, Clone)]
pub enum RunResult {
    Finished(Option<Value>),
    Breakpoint,
    Awaiting(Arc<TokioMutex<PromiseState>>),
}

#[derive(Debug, Clone)]
pub enum ExecutionResult {
    Continue,
    Breakpoint,
    Awaiting(Arc<TokioMutex<PromiseState>>),
}

/// Opcodes for the registry-based VM
/// 
/// Registry-based instruction format:
/// - Most instructions use explicit register operands (Rd, Rs, etc.)
/// - No implicit stack operations
/// - Fixed register file per call frame
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Nop = 0x00,

    // Load constants into registers
    LoadConst = 0x10,  // Rd, string_idx
    LoadInt = 0x11,    // Rd, 8 bytes
    LoadFloat = 0x12,  // Rd, 8 bytes
    LoadBool = 0x13,   // Rd, 1 byte
    LoadNull = 0x14,   // Rd

    // Register-to-register operations
    Move = 0x20,       // Rd, Rs

    // Local variable operations
    LoadLocal = 0x21,  // Rd, name_idx
    StoreLocal = 0x22, // name_idx, Rs

    // Property access
    GetProperty = 0x30,  // Rd, Robj, name_idx
    SetProperty = 0x31,  // Robj, name_idx, Rs

    // Function calls
    Call = 0x40,         // Rd, func_idx, arg_start, arg_count
    CallNative = 0x41,   // Rd, name_idx, arg_start, arg_count
    Invoke = 0x42,       // Rd, method_idx, arg_start, arg_count
    Return = 0x43,       // Rs
    CallAsync = 0x44,
    CallNativeAsync = 0x45,
    InvokeAsync = 0x46,
    Await = 0x47,
    Spawn = 0x48,
    InvokeInterface = 0x49,  // Rd, vtable_idx, arg_start, arg_count
    InvokeInterfaceAsync = 0x4A,  // Rd, vtable_idx, arg_start, arg_count
    
    // Indexed native calls (optimized - uses function index instead of string lookup)
    CallNativeIndexed = 0x4B,  // Rd, func_idx (u16), arg_start, arg_count
    CallNativeIndexedAsync = 0x4C,  // Rd, func_idx (u16), arg_start, arg_count

    // Control flow
    Jump = 0x50,         // target (2 bytes)
    JumpIfTrue = 0x51,   // Rs, target (2 bytes)
    JumpIfFalse = 0x52,  // Rs, target (2 bytes)

    // Comparisons (3-register format: Rd = Rs1 op Rs2)
    Equal = 0x60,    // Rd, Rs1, Rs2
    NotEqual = 0x61, // Rd, Rs1, Rs2
    Greater = 0x66,  // Rd, Rs1, Rs2
    Less = 0x67,     // Rd, Rs1, Rs2
    GreaterEqual = 0x6A,
    LessEqual = 0x6B,

    // Logical operations
    And = 0x62,      // Rd, Rs1, Rs2
    Or = 0x63,       // Rd, Rs1, Rs2
    Not = 0x64,      // Rd, Rs

    // Arithmetic (3-register format)
    Add = 0x68,      // Rd, Rs1, Rs2
    Subtract = 0x69, // Rd, Rs1, Rs2
    Multiply = 0x70, // Rd, Rs1, Rs2
    Divide = 0x71,   // Rd, Rs1, Rs2
    Modulo = 0x75,   // Rd, Rs1, Rs2

    // String operations
    Concat = 0x65,   // Rd, rs_start, count

    // Type operations
    Cast = 0x74,     // Rd, Rs, type
    Array = 0x76,    // Rd, rs_start, count
    Index = 0x77,    // Rd, Robj, Ridx

    // Debugging
    Line = 0x73,     // line_number (2 bytes)

    // Exception handling
    TryStart = 0x80, // catch_pc (2 bytes), catch_reg
    TryEnd = 0x81,
    Throw = 0x82,    // Rs

    // Debugging
    Breakpoint = 0x90,

    // Execution control
    Halt = 0xFF,
}

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
            Value::Promise(_) => serializer.serialize_none(),

            Value::Exception(e) => serializer.serialize_str(&e.message),
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
pub struct VmState {
    pub locals: HashMap<String, Value>,
    pub classes: HashMap<String, Class>,
    pub functions: HashMap<String, Function>,
}

impl VM {
    /// Create a snapshot of the current VM state
    pub fn snapshot(&self) -> VmState {
        VmState {
            locals: self.locals.clone(),
            classes: self.classes.clone(),
            functions: self.functions.clone(),
        }
    }

    /// Restore the VM to a previous state
    pub fn restore(&mut self, state: &VmState) {
        self.locals = state.locals.clone();
        self.classes = state.classes.clone();
        self.functions = state.functions.clone();
    }
}

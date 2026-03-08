use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Serialize, Deserialize, Serializer, Deserializer};
use serde::ser::SerializeMap;
use serde::de::{MapAccess, Visitor};
use std::fmt;

pub type Bytecode = Vec<u8>;

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
    Int8(i8), // FFI types
    Int16(i16), // FFI types
    Int32(i32),
    Int64(i64),
    UInt8(u8), // FFI types
    UInt16(u16), // FFI types
    UInt32(u32),
    UInt64(u64),
    Float32(f32),
    Float64(f64),
    Bool(bool),
    Null,
    Instance(Instance),
    Promise(Arc<Mutex<PromiseState>>),
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::String(a), Value::String(b)) => a == b,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Null, Value::Null) => true,
            (Value::Instance(a), Value::Instance(b)) => a.class == b.class,
            (Value::Promise(a), Value::Promise(b)) => Arc::ptr_eq(a, b),
            // Compare all integer types by converting to i64
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
            // Compare all float types by converting to f64
            (Value::Float64(a), Value::Float64(b)) => a == b,
            (Value::Float64(a), Value::Float32(b)) => *a == *b as f64,
            (Value::Float32(a), Value::Float64(b)) => *a as f64 == *b,
            (Value::Float32(a), Value::Float32(b)) => a == b,
            // Cross-type numeric comparison (int vs float)
            (Value::Int64(a), Value::Float64(b)) => (*a as f64) == *b,
            (Value::Float64(a), Value::Int64(b)) => *a == (*b as f64),
            _ => false,
        }
    }
}

impl Value {
    /// Convert any integer value to i64 (primary Bengal integer type)
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

    /// Convert any float value to f64
    pub fn to_f64(&self) -> Option<f64> {
        match self {
            Value::Float64(n) => Some(*n),
            Value::Float32(n) => Some(*n as f64),
            _ => None,
        }
    }

    /// Check if value is an integer type >= 32 bits (suitable for arithmetic)
    pub fn is_arithmetic_int(&self) -> bool {
        matches!(self, Value::Int64(_) | Value::Int32(_) | Value::UInt32(_) | Value::UInt64(_))
    }

    /// Check if value is a float type (suitable for arithmetic)
    pub fn is_arithmetic_float(&self) -> bool {
        matches!(self, Value::Float64(_) | Value::Float32(_))
    }

    /// Convert arithmetic integer value to i64 (only 32-bit and larger types)
    pub fn to_arithmetic_int(&self) -> Option<i64> {
        match self {
            Value::Int64(n) => Some(*n),
            Value::Int32(n) => Some(*n as i64),
            Value::UInt32(n) => Some(*n as i64),
            Value::UInt64(n) => Some(*n as i64),
            _ => None,
        }
    }

    /// Convert any numeric value to i64, truncating floats (for FFI - all types)
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

    /// Convert any numeric value to f64 (for FFI - all types)
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

    /// Convert to u8 for FFI
    pub fn to_u8(&self) -> Option<u8> {
        self.to_int().map(|n| n as u8)
    }

    /// Convert to i8 for FFI
    pub fn to_i8(&self) -> Option<i8> {
        self.to_int().map(|n| n as i8)
    }

    /// Convert to u16 for FFI
    pub fn to_u16(&self) -> Option<u16> {
        self.to_int().map(|n| n as u16)
    }

    /// Convert to i16 for FFI
    pub fn to_i16(&self) -> Option<i16> {
        self.to_int().map(|n| n as i16)
    }

    /// Convert to u32 for FFI
    pub fn to_u32(&self) -> Option<u32> {
        self.to_int().map(|n| n as u32)
    }

    /// Convert to i32 for FFI
    pub fn to_i32(&self) -> Option<i32> {
        self.to_int().map(|n| n as i32)
    }

    /// Convert to u64 for FFI
    pub fn to_u64(&self) -> Option<u64> {
        self.to_int().map(|n| n as u64)
    }

    /// Convert to f32 for FFI
    pub fn to_f32(&self) -> Option<f32> {
        self.to_float().map(|n| n as f32)
    }

    /// Check if value is truthy
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

    /// Convert any value to string
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
            Value::Instance(_) => "[instance]".to_string(),
            Value::Promise(_) => "[promise]".to_string(),
        }
    }
}

#[derive(Clone)]
pub enum PromiseState {
    Pending,
    Resolved(Value),
    Rejected(String),
}

#[derive(Clone)]
pub struct Instance {
    pub class: String,
    pub fields: HashMap<String, Value>,
}

#[derive(Clone)]
pub struct Class {
    pub name: String,
    pub fields: HashMap<String, Value>,
    pub methods: HashMap<String, Method>,
}

#[derive(Clone)]
pub struct Method {
    pub name: String,
    pub bytecode: Vec<u8>,
}

pub type NativeFn = fn(&mut Vec<Value>) -> Result<Value, String>;

pub struct VM {
    memory: Bytecode,
    stack: Vec<Value>,
    pc: usize,
    strings: Vec<String>,
    locals: HashMap<String, Value>,
    classes: HashMap<String, Class>,
    pub native_functions: HashMap<String, NativeFn>,
    pub fallback_native: Option<NativeFn>,
}

impl VM {
    pub fn new() -> Self {
        Self {
            memory: Vec::new(),
            stack: Vec::new(),
            pc: 0,
            strings: Vec::new(),
            locals: HashMap::new(),
            classes: HashMap::new(),
            native_functions: HashMap::new(),
            fallback_native: None,
        }
    }

    pub fn register_native(&mut self, name: &str, f: NativeFn) {
        self.native_functions.insert(name.to_string(), f);
    }

    pub fn register_fallback(&mut self, f: NativeFn) {
        self.fallback_native = Some(f);
    }

    pub fn load(&mut self, bytecode: &[u8], strings: Vec<String>, classes: Vec<Class>) -> Result<(), String> {
        self.memory = bytecode.to_vec();
        self.strings = strings;
        self.classes.clear();
        for class in classes {
            self.classes.insert(class.name.clone(), class);
        }
        self.pc = 0;
        self.stack.clear();
        Ok(())
    }

    pub async fn run(&mut self) -> Result<Option<Value>, String> {
        while self.pc < self.memory.len() {
            let opcode = self.memory[self.pc];
            let result = self.execute(opcode).await?;

            if opcode == Opcode::Halt as u8 {
                break;
            }

            if let ExecutionResult::Awaiting(promise) = result {
                return Ok(Some(Value::Promise(promise)));
            }

            self.pc += 1;
        }
        
        Ok(self.stack.last().cloned())
    }

    async fn execute(&mut self, opcode: u8) -> Result<ExecutionResult, String> {
        match opcode {
            x if x == Opcode::Nop as u8 => {}

            x if x == Opcode::PushString as u8 => {
                self.pc += 1;
                let idx = self.memory[self.pc] as usize;
                let s = self.strings.get(idx)
                    .ok_or(format!("Invalid string index: {}", idx))?
                    .clone();
                self.stack.push(Value::String(s));
            }

            x if x == Opcode::PushInt as u8 => {
                self.pc += 1;
                let bytes: [u8; 8] = self.memory[self.pc..self.pc + 8]
                    .try_into()
                    .map_err(|_| "Invalid int encoding")?;
                let n = i64::from_le_bytes(bytes);
                self.stack.push(Value::Int64(n));
                self.pc += 7;
            }

            x if x == Opcode::PushFloat as u8 => {
                self.pc += 1;
                let bytes: [u8; 8] = self.memory[self.pc..self.pc + 8]
                    .try_into()
                    .map_err(|_| "Invalid float encoding")?;
                let n = f64::from_le_bytes(bytes);
                self.stack.push(Value::Float64(n));
                self.pc += 7;
            }

            x if x == Opcode::PushBool as u8 => {
                self.pc += 1;
                let b = self.memory[self.pc] != 0;
                self.stack.push(Value::Bool(b));
            }

            x if x == Opcode::PushNull as u8 => {
                self.stack.push(Value::Null);
            }

            x if x == Opcode::LoadLocal as u8 => {
                self.pc += 1;
                let idx = self.memory[self.pc] as usize;
                let name = self.strings.get(idx)
                    .ok_or(format!("Invalid string index: {}", idx))?
                    .clone();
                let value = self.locals.get(&name)
                    .cloned()
                    .unwrap_or(Value::Null);
                self.stack.push(value);
            }

            x if x == Opcode::StoreLocal as u8 => {
                self.pc += 1;
                let idx = self.memory[self.pc] as usize;
                let name = self.strings.get(idx)
                    .ok_or(format!("Invalid string index: {}", idx))?
                    .clone();
                if let Some(value) = self.stack.pop() {
                    self.locals.insert(name, value);
                }
            }

            x if x == Opcode::GetProperty as u8 => {
                self.pc += 1;
                let idx = self.memory[self.pc] as usize;
                let name = self.strings.get(idx)
                    .ok_or(format!("Invalid string index: {}", idx))?
                    .clone();

                if let Some(Value::Instance(instance)) = self.stack.pop() {
                    let value = instance.fields.get(&name)
                        .cloned()
                        .unwrap_or(Value::Null);
                    self.stack.push(value);
                } else {
                    return Err("Expected instance for property get".to_string());
                }
            }

            x if x == Opcode::SetProperty as u8 => {
                self.pc += 1;
                let idx = self.memory[self.pc] as usize;
                let name = self.strings.get(idx)
                    .ok_or(format!("Invalid string index: {}", idx))?
                    .clone();

                let value = self.stack.pop();
                if let Some(Value::Instance(mut instance)) = self.stack.pop() {
                    if let Some(v) = value {
                        instance.fields.insert(name, v);
                    }
                    self.stack.push(Value::Instance(instance));
                } else {
                    return Err("Expected instance for property set".to_string());
                }
            }

            x if x == Opcode::Call as u8 => {
                self.pc += 1;
                let func_idx = self.memory[self.pc] as usize;
                self.pc += 1;
                let arg_count = self.memory[self.pc] as usize;

                let func_name = self.strings.get(func_idx)
                    .ok_or(format!("Invalid function index: {}", func_idx))?
                    .clone();

                if let Some(class) = self.classes.get(&func_name).cloned() {
                    for _ in 0..arg_count {
                        self.stack.pop();
                    }
                    let instance = Instance {
                        class: func_name,
                        fields: class.fields.clone(),
                    };
                    self.stack.push(Value::Instance(instance));
                } else if let Some(native_f) = self.native_functions.get(&func_name) {
                    let mut args = Vec::new();
                    for _ in 0..arg_count {
                        if let Some(val) = self.stack.pop() {
                            args.push(val);
                        }
                    }
                    args.reverse();
                    let result = native_f(&mut args)?;
                    self.stack.push(result);
                } else {
                    for _ in 0..arg_count {
                        self.stack.pop();
                    }
                    self.stack.push(Value::Null);
                }
            }

            x if x == Opcode::CallAsync as u8 => {
                self.pc += 1;
                let func_idx = self.memory[self.pc] as usize;
                self.pc += 1;
                let arg_count = self.memory[self.pc] as usize;

                let _func_name = self.strings.get(func_idx)
                    .ok_or(format!("Invalid function index: {}", func_idx))?
                    .clone();

                for _ in 0..arg_count {
                    self.stack.pop();
                }

                let promise = Arc::new(Mutex::new(PromiseState::Resolved(Value::Null)));
                self.stack.push(Value::Promise(promise));
            }

            x if x == Opcode::CallNative as u8 || x == Opcode::CallNativeAsync as u8 => {
                self.pc += 1;
                let name_idx = self.memory[self.pc] as usize;
                self.pc += 1;
                let arg_count = self.memory[self.pc] as usize;

                let name = self.strings.get(name_idx)
                    .ok_or(format!("Invalid native name index: {}", name_idx))?
                    .clone();

                let mut args = Vec::new();
                for _ in 0..arg_count {
                    if let Some(val) = self.stack.pop() {
                        args.push(val);
                    }
                }
                args.reverse();

                let func = self.native_functions.get(&name);
                let result = match func {
                    Some(f) => f(&mut args)?,
                    None => {
                        match &self.fallback_native {
                            Some(f) => f(&mut args)?,
                            None => {
                                return Err(format!("Native function not found: {}", name));
                            }
                        }
                    }
                };
                self.stack.push(result);
            }

            x if x == Opcode::Invoke as u8 => {
                self.pc += 1;
                let method_idx = self.memory[self.pc] as usize;
                self.pc += 1;
                let arg_count = self.memory[self.pc] as usize;

                let _method_name = self.strings.get(method_idx)
                    .ok_or(format!("Invalid method index: {}", method_idx))?
                    .clone();

                for _ in 0..arg_count {
                    self.stack.pop();
                }

                self.stack.push(Value::Null);
            }

            x if x == Opcode::InvokeAsync as u8 => {
                self.pc += 1;
                let method_idx = self.memory[self.pc] as usize;
                self.pc += 1;
                let arg_count = self.memory[self.pc] as usize;

                let _method_name = self.strings.get(method_idx)
                    .ok_or(format!("Invalid method index: {}", method_idx))?
                    .clone();

                for _ in 0..arg_count {
                    self.stack.pop();
                }

                let promise = Arc::new(Mutex::new(PromiseState::Resolved(Value::Null)));
                self.stack.push(Value::Promise(promise));
            }

            x if x == Opcode::Await as u8 => {
                if let Some(value) = self.stack.pop() {
                    match value {
                        Value::Promise(promise) => {
                            let mut state = promise.lock().await;
                            match &mut *state {
                                PromiseState::Pending => {
                                    self.stack.push(Value::Promise(promise.clone()));
                                    drop(state);
                                    return Ok(ExecutionResult::Awaiting(promise));
                                }
                                PromiseState::Resolved(v) => {
                                    self.stack.push(v.clone());
                                }
                                PromiseState::Rejected(e) => {
                                    return Err(format!("Promise rejected: {}", e));
                                }
                            }
                        }
                        _ => {
                            return Err("Can only await Promise values".to_string());
                        }
                    }
                } else {
                    return Err("Stack underflow during await".to_string());
                }
            }

            x if x == Opcode::Return as u8 => {
                // Return from current frame
            }

            x if x == Opcode::Jump as u8 => {
                self.pc += 1;
                let target = self.memory[self.pc] as usize;
                self.pc = target.saturating_sub(1);
            }

            x if x == Opcode::JumpIfTrue as u8 => {
                self.pc += 1;
                let target = self.memory[self.pc] as usize;
                if let Some(Value::Bool(true)) = self.stack.last() {
                    self.pc = target.saturating_sub(1);
                }
            }

            x if x == Opcode::JumpIfFalse as u8 => {
                self.pc += 1;
                let target = self.memory[self.pc] as usize;
                let should_jump = match self.stack.last() {
                    Some(Value::Bool(false)) => true,
                    Some(Value::Null) => true,
                    _ => false,
                };
                if should_jump {
                    self.pc = target.saturating_sub(1);
                }
            }

            x if x == Opcode::JumpIfGreater as u8 => {
                self.pc += 1;
                let target = self.memory[self.pc] as usize;
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let should_jump = match (&left, &right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        left.to_arithmetic_int().unwrap() > right.to_arithmetic_int().unwrap()
                    }
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        left.to_float().unwrap() > right.to_float().unwrap()
                    }
                    _ => false,
                };
                if should_jump {
                    self.pc = target.saturating_sub(1);
                }
            }

            x if x == Opcode::JumpIfLess as u8 => {
                self.pc += 1;
                let target = self.memory[self.pc] as usize;
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let should_jump = match (&left, &right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        left.to_arithmetic_int().unwrap() < right.to_arithmetic_int().unwrap()
                    }
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        left.to_float().unwrap() < right.to_float().unwrap()
                    }
                    _ => false,
                };
                if should_jump {
                    self.pc = target.saturating_sub(1);
                }
            }

            x if x == Opcode::Equal as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = left == right;
                self.stack.push(Value::Bool(result));
            }

            x if x == Opcode::Not as u8 => {
                if let Some(Value::Bool(b)) = self.stack.pop() {
                    self.stack.push(Value::Bool(!b));
                } else {
                    self.stack.push(Value::Bool(true));
                }
            }

            x if x == Opcode::And as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = left.is_truthy() && right.is_truthy();
                self.stack.push(Value::Bool(result));
            }

            x if x == Opcode::Or as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = left.is_truthy() || right.is_truthy();
                self.stack.push(Value::Bool(result));
            }

            x if x == Opcode::Greater as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Bool(left.to_arithmetic_int().unwrap() > right.to_arithmetic_int().unwrap())
                    }
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        Value::Bool(left.to_float().unwrap() > right.to_float().unwrap())
                    }
                    _ => Value::Bool(false),
                };
                self.stack.push(result);
            }

            x if x == Opcode::Less as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Bool(left.to_arithmetic_int().unwrap() < right.to_arithmetic_int().unwrap())
                    }
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        Value::Bool(left.to_float().unwrap() < right.to_float().unwrap())
                    }
                    _ => Value::Bool(false),
                };
                self.stack.push(result);
            }

            x if x == Opcode::Add as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    // String concatenation
                    (Value::String(a), Value::String(b)) => Value::String(a.clone() + b),
                    // Both integers >= 32 bits - result is i64
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() + right.to_arithmetic_int().unwrap())
                    }
                    // Both floats - result is f64
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        Value::Float64(left.to_float().unwrap() + right.to_float().unwrap())
                    }
                    // Mixed int/float - promote to f64
                    _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                         (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                        let left_f = left.to_float().unwrap();
                        let right_f = right.to_float().unwrap();
                        Value::Float64(left_f + right_f)
                    }
                    _ => Value::Null,
                };
                self.stack.push(result);
            }

            x if x == Opcode::Subtract as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    // Both integers >= 32 bits - result is i64
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() - right.to_arithmetic_int().unwrap())
                    }
                    // Both floats - result is f64
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        Value::Float64(left.to_float().unwrap() - right.to_float().unwrap())
                    }
                    // Mixed int/float - promote to f64
                    _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                         (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                        let left_f = left.to_float().unwrap();
                        let right_f = right.to_float().unwrap();
                        Value::Float64(left_f - right_f)
                    }
                    _ => Value::Null,
                };
                self.stack.push(result);
            }

            x if x == Opcode::Multiply as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    // Both integers >= 32 bits - result is i64
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        Value::Int64(left.to_arithmetic_int().unwrap() * right.to_arithmetic_int().unwrap())
                    }
                    // Both floats - result is f64
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        Value::Float64(left.to_float().unwrap() * right.to_float().unwrap())
                    }
                    // Mixed int/float - promote to f64
                    _ if (left.is_arithmetic_int() && right.is_arithmetic_float()) ||
                         (left.is_arithmetic_float() && right.is_arithmetic_int()) => {
                        let left_f = left.to_float().unwrap();
                        let right_f = right.to_float().unwrap();
                        Value::Float64(left_f * right_f)
                    }
                    _ => Value::Null,
                };
                self.stack.push(result);
            }

            x if x == Opcode::Divide as u8 => {
                let right = self.stack.pop().unwrap_or(Value::Null);
                let left = self.stack.pop().unwrap_or(Value::Null);
                let result = match (&left, &right) {
                    // Both integers >= 32 bits - integer division, result is i64
                    _ if left.is_arithmetic_int() && right.is_arithmetic_int() => {
                        let r = right.to_arithmetic_int().unwrap();
                        if r != 0 {
                            Value::Int64(left.to_arithmetic_int().unwrap() / r)
                        } else {
                            Value::Null
                        }
                    }
                    // Both floats - float division, result is f64
                    _ if left.is_arithmetic_float() && right.is_arithmetic_float() => {
                        let r = right.to_float().unwrap();
                        if r != 0.0 {
                            Value::Float64(left.to_float().unwrap() / r)
                        } else {
                            Value::Null
                        }
                    }
                    // Mixed int/float - promote to f64
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
                self.stack.push(result);
            }

            x if x == Opcode::Concat as u8 => {
                self.pc += 1;
                let count = self.memory[self.pc] as usize;

                let mut result = String::new();
                for _ in 0..count {
                    if let Some(value) = self.stack.pop() {
                        result = value.to_string() + &result;
                    }
                }
                self.stack.push(Value::String(result));
            }

            x if x == Opcode::Pop as u8 => {
                self.stack.pop();
            }

            x if x == Opcode::Halt as u8 => {}

            _ => {
                return Err(format!("Unknown opcode: 0x{:02X}", opcode));
            }
        }
        Ok(ExecutionResult::Continue)
    }
}

pub enum ExecutionResult {
    Continue,
    Awaiting(Arc<Mutex<PromiseState>>),
}

impl Default for VM {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(u8)]
#[allow(dead_code)]
pub enum Opcode {
    Nop = 0x00,

    PushString = 0x10,
    PushInt = 0x11,
    PushFloat = 0x12,
    PushBool = 0x13,
    PushNull = 0x14,

    LoadLocal = 0x20,
    StoreLocal = 0x21,

    GetProperty = 0x30,
    SetProperty = 0x31,

    Call = 0x40,
    CallNative = 0x41,
    Invoke = 0x42,
    Return = 0x43,
    CallAsync = 0x44,
    CallNativeAsync = 0x45,
    InvokeAsync = 0x46,
    Await = 0x47,
    Spawn = 0x48,

    Jump = 0x50,
    JumpIfTrue = 0x51,
    JumpIfFalse = 0x52,
    JumpIfGreater = 0x53,
    JumpIfLess = 0x54,

    Equal = 0x60,
    NotEqual = 0x61,
    And = 0x62,
    Or = 0x63,
    Not = 0x64,
    Concat = 0x65,
    Greater = 0x66,
    Less = 0x67,
    Add = 0x68,
    Subtract = 0x69,
    Multiply = 0x70,
    Divide = 0x71,

    Pop = 0x72,

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
                let mut map = serializer.serialize_map(Some(inst.fields.len()))?;
                for (k, v) in &inst.fields {
                    map.serialize_entry(k, v)?;
                }
                map.end()
            }
            Value::Promise(_) => serializer.serialize_none(),
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

            fn visit_none<E>(self) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Null)
            }
            
            fn visit_unit<E>(self) -> Result<Value, E>
            where E: serde::de::Error {
                Ok(Value::Null)
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Value, A::Error>
            where A: serde::de::SeqAccess<'de> {
                let mut fields = HashMap::new();
                let mut idx = 0;
                while let Some(value) = seq.next_element()? {
                    fields.insert(idx.to_string(), value);
                    idx += 1;
                }
                fields.insert("length".to_string(), Value::Int64(idx));
                Ok(Value::Instance(Instance {
                    class: "Array".to_string(),
                    fields,
                }))
            }

            fn visit_map<A>(self, mut map: A) -> Result<Value, A::Error>
            where A: MapAccess<'de> {
                let mut fields = HashMap::new();
                while let Some((key, value)) = map.next_entry()? {
                    fields.insert(key, value);
                }
                Ok(Value::Instance(Instance {
                    class: "Object".to_string(),
                    fields,
                }))
            }
        }

        deserializer.deserialize_any(ValueVisitor)
    }
}


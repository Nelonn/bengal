//! VERY EXPERIMENTAL LLVM Backend
//! 
//! This backend generates LLVM IR text representation.
//! Still very experimental, but now actually generates some IR!
//! 
//! This module is optional. Enable the `llvm` feature to use it.

#![cfg(feature = "llvm")]

use std::collections::HashMap;
use std::fmt::Write;

/// LLVM IR type representation
#[derive(Debug, Clone)]
pub enum LlvmType {
    Void,
    I1,
    I32,
    I64,
    F32,
    F64,
    Pointer(Box<LlvmType>),
    Array(u64, Box<LlvmType>),
    Struct(Vec<LlvmType>),
}

impl LlvmType {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlvmType::Void => "void",
            LlvmType::I1 => "i1",
            LlvmType::I32 => "i32",
            LlvmType::I64 => "i64",
            LlvmType::F32 => "float",
            LlvmType::F64 => "double",
            LlvmType::Pointer(_) => "ptr",
            LlvmType::Array(_, _) => "array",
            LlvmType::Struct(_) => "struct",
        }
    }
}

/// LLVM IR value representation
#[derive(Debug, Clone)]
pub enum LlvmValue {
    /// Register value (e.g., %0, %1)
    Register(u32),
    /// Constant integer
    IntConst(i64),
    /// Constant float
    FloatConst(f64),
    /// Constant boolean
    BoolConst(bool),
    /// Constant string (global reference)
    StringConst(String),
    /// Alloca reference (stack allocation)
    Alloca(String),
    /// Function parameter
    Param(u32),
    /// Global variable
    Global(String),
}

impl LlvmValue {
    pub fn as_str(&self) -> String {
        match self {
            LlvmValue::Register(n) => format!("%{}", n),
            LlvmValue::IntConst(n) => format!("{}", n),
            LlvmValue::FloatConst(n) => format!("{}", n),
            LlvmValue::BoolConst(true) => "true".to_string(),
            LlvmValue::BoolConst(false) => "false".to_string(),
            LlvmValue::StringConst(s) => format!("@{}", s),
            LlvmValue::Alloca(name) => format!("%{}", name),
            LlvmValue::Param(n) => format!("%{}", n),
            LlvmValue::Global(name) => format!("@{}", name),
        }
    }
}

/// LLVM IR instruction
#[derive(Debug, Clone)]
pub enum LlvmInstr {
    /// Binary arithmetic operations (lhs, rhs, type, result_reg)
    Add(LlvmValue, LlvmValue, LlvmType, u32),
    Sub(LlvmValue, LlvmValue, LlvmType, u32),
    Mul(LlvmValue, LlvmValue, LlvmType, u32),
    SDiv(LlvmValue, LlvmValue, LlvmType, u32),
    UDiv(LlvmValue, LlvmValue, LlvmType, u32),
    FAdd(LlvmValue, LlvmValue, LlvmType, u32),
    FSub(LlvmValue, LlvmValue, LlvmType, u32),
    FMul(LlvmValue, LlvmValue, LlvmType, u32),
    FDiv(LlvmValue, LlvmValue, LlvmType, u32),

    /// Comparison operations
    ICmp(LlvmCompareOp, LlvmValue, LlvmValue),
    FCmp(LlvmCompareOp, LlvmValue, LlvmValue),

    /// Logical operations
    And(LlvmValue, LlvmValue),
    Or(LlvmValue, LlvmValue),
    Xor(LlvmValue, LlvmValue),
    Not(LlvmValue),

    /// Memory operations (type, optional_size, result_reg)
    Alloca(LlvmType, Option<u32>, u32),
    Load(LlvmType, LlvmValue, u32),
    Store(LlvmType, LlvmValue, LlvmValue),  // value, ptr

    /// Control flow
    Br(String),                    // Unconditional branch
    CondBr(LlvmValue, String, String),  // Conditional branch
    Ret(Option<LlvmValue>, LlvmType),

    /// Function call (type, name, args, result_reg)
    Call(LlvmType, String, Vec<LlvmValue>, u32),

    /// Phi node (for SSA)
    Phi(LlvmType, Vec<(LlvmValue, String)>),

    /// Cast operations
    ZExt(LlvmValue, LlvmType, LlvmType),  // Zero extend
    SExt(LlvmValue, LlvmType, LlvmType),  // Sign extend
    FpToSi(LlvmValue, LlvmType, LlvmType), // Float to signed int
    SiToFp(LlvmValue, LlvmType, LlvmType), // Signed int to float
    BitCast(LlvmValue, LlvmType, LlvmType),

    /// Get element pointer
    GetElementPtr(LlvmType, LlvmValue, Vec<LlvmValue>),
}

#[derive(Debug, Clone, Copy)]
pub enum LlvmCompareOp {
    Eq, Ne,
    Sgt, Sge, Slt, Sle,
    Ugt, Uge, Ult, Ule,
    Oeq, One, Ogt, Oge, Olt, Ole,
    Ueq, Une,
}

impl LlvmCompareOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            LlvmCompareOp::Eq => "eq",
            LlvmCompareOp::Ne => "ne",
            LlvmCompareOp::Sgt => "sgt",
            LlvmCompareOp::Sge => "sge",
            LlvmCompareOp::Slt => "slt",
            LlvmCompareOp::Sle => "sle",
            LlvmCompareOp::Ugt => "ugt",
            LlvmCompareOp::Uge => "uge",
            LlvmCompareOp::Ult => "ult",
            LlvmCompareOp::Ule => "ule",
            LlvmCompareOp::Oeq => "oeq",
            LlvmCompareOp::One => "one",
            LlvmCompareOp::Ogt => "ogt",
            LlvmCompareOp::Oge => "oge",
            LlvmCompareOp::Olt => "olt",
            LlvmCompareOp::Ole => "ole",
            LlvmCompareOp::Ueq => "ueq",
            LlvmCompareOp::Une => "une",
        }
    }
}

/// LLVM Basic Block
#[derive(Debug, Clone)]
pub struct LlvmBasicBlock {
    pub name: String,
    pub instructions: Vec<LlvmInstr>,
    pub terminator: Option<LlvmInstr>,
}

impl LlvmBasicBlock {
    pub fn new(name: String) -> Self {
        Self {
            name,
            instructions: Vec::new(),
            terminator: None,
        }
    }
}

/// LLVM Function
#[derive(Debug, Clone)]
pub struct LlvmFunction {
    pub name: String,
    pub return_type: LlvmType,
    pub param_types: Vec<LlvmType>,
    pub blocks: Vec<LlvmBasicBlock>,
    pub is_external: bool,
}

impl LlvmFunction {
    pub fn new(name: String, return_type: LlvmType, param_types: Vec<LlvmType>) -> Self {
        Self {
            name,
            return_type,
            param_types,
            blocks: Vec::new(),
            is_external: false,
        }
    }
    
    pub fn external(name: String, return_type: LlvmType, param_types: Vec<LlvmType>) -> Self {
        let mut f = Self::new(name, return_type, param_types);
        f.is_external = true;
        f
    }
}

/// LLVM Module - the top-level IR container
#[derive(Debug)]
pub struct LlvmModule {
    pub name: String,
    pub functions: Vec<LlvmFunction>,
    pub globals: HashMap<String, (LlvmType, Option<LlvmValue>)>,
    next_reg: u32,
}

impl LlvmModule {
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: Vec::new(),
            globals: HashMap::new(),
            next_reg: 0,
        }
    }
    
    pub fn add_function(&mut self, func: LlvmFunction) {
        self.functions.push(func);
    }
    
    pub fn add_global(&mut self, name: String, ty: LlvmType, init: Option<LlvmValue>) {
        self.globals.insert(name, (ty, init));
    }
    
    fn next_register(&mut self) -> u32 {
        let reg = self.next_reg;
        self.next_reg += 1;
        reg
    }
}

/// LLVM IR Builder - helps construct IR incrementally
pub struct LlvmBuilder {
    module: LlvmModule,
    current_function: Option<String>,
    current_block: Option<String>,
    variables: HashMap<String, LlvmValue>,
    block_index: HashMap<String, usize>,
    next_reg: u32,
}

impl LlvmBuilder {
    pub fn new(module_name: &str) -> Self {
        Self {
            module: LlvmModule::new(module_name.to_string()),
            current_function: None,
            current_block: None,
            variables: HashMap::new(),
            block_index: HashMap::new(),
            next_reg: 0,
        }
    }
    
    /// Start building a function
    pub fn begin_function(&mut self, name: &str, return_type: LlvmType, param_types: Vec<LlvmType>) {
        let func = LlvmFunction::new(name.to_string(), return_type, param_types.clone());
        self.module.add_function(func);
        self.current_function = Some(name.to_string());
        self.variables.clear();
        
        // Assign parameters to variables
        for (i, _ty) in param_types.iter().enumerate() {
            let param_name = format!("param{}", i);
            self.variables.insert(param_name, LlvmValue::Param(i as u32));
        }
    }
    
    /// End the current function
    pub fn end_function(&mut self) {
        self.current_function = None;
        self.current_block = None;
    }
    
    /// Begin a basic block
    pub fn begin_block(&mut self, name: &str) {
        self.current_block = Some(name.to_string());
        if let Some(func_name) = &self.current_function {
            let func_idx = self.module.functions.iter()
                .position(|f| &f.name == func_name)
                .unwrap();
            let block = LlvmBasicBlock::new(name.to_string());
            self.module.functions[func_idx].blocks.push(block);
        }
    }
    
    /// End the current block
    pub fn end_block(&mut self) {
        self.current_block = None;
    }
    
    /// Generate an add instruction
    pub fn add(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Add(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a sub instruction
    pub fn sub(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Sub(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a mul instruction
    pub fn mul(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Mul(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a sdiv instruction
    pub fn sdiv(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::SDiv(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a fadd instruction
    pub fn fadd(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::FAdd(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a fsub instruction
    pub fn fsub(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::FSub(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a fmul instruction
    pub fn fmul(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::FMul(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate a fdiv instruction
    pub fn fdiv(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::FDiv(lhs, rhs, ty, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Generate an alloca instruction (stack allocation)
    pub fn alloca(&mut self, ty: LlvmType, name: &str) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Alloca(ty, None, reg);
        self.emit(instr);
        let value = LlvmValue::Alloca(format!("{}_{}", name, reg));
        self.variables.insert(name.to_string(), value.clone());
        value
    }

    /// Generate a load instruction
    pub fn load(&mut self, ty: LlvmType, ptr: LlvmValue) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Load(ty, ptr, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }

    /// Generate a store instruction
    pub fn store(&mut self, ty: LlvmType, value: LlvmValue, ptr: LlvmValue) {
        let instr = LlvmInstr::Store(ty, value, ptr);
        self.emit(instr);
    }

    /// Generate a return instruction
    pub fn ret(&mut self, value: Option<LlvmValue>, ty: LlvmType) {
        let instr = LlvmInstr::Ret(value, ty);
        self.emit_terminator(instr);
    }

    /// Generate a branch instruction
    pub fn br(&mut self, target: &str) {
        let instr = LlvmInstr::Br(target.to_string());
        self.emit_terminator(instr);
    }

    /// Generate a conditional branch instruction
    pub fn cond_br(&mut self, cond: LlvmValue, then_block: &str, else_block: &str) {
        let instr = LlvmInstr::CondBr(cond, then_block.to_string(), else_block.to_string());
        self.emit_terminator(instr);
    }

    /// Generate a call instruction
    pub fn call(&mut self, return_type: LlvmType, func_name: &str, args: Vec<LlvmValue>) -> LlvmValue {
        let reg = self.next_reg;
        self.next_reg += 1;
        let instr = LlvmInstr::Call(return_type, func_name.to_string(), args, reg);
        self.emit(instr);
        LlvmValue::Register(reg)
    }
    
    /// Store a variable
    pub fn store_variable(&mut self, name: &str, value: LlvmValue) {
        self.variables.insert(name.to_string(), value);
    }
    
    /// Load a variable
    pub fn load_variable(&self, name: &str) -> Option<LlvmValue> {
        self.variables.get(name).cloned()
    }
    
    fn emit(&mut self, instr: LlvmInstr) {
        if let Some(func_name) = &self.current_function {
            if let Some(func_idx) = self.module.functions.iter().position(|f| &f.name == func_name) {
                if let Some(block_name) = &self.current_block {
                    if let Some(block_idx) = self.module.functions[func_idx].blocks.iter()
                        .position(|b| &b.name == block_name)
                    {
                        self.module.functions[func_idx].blocks[block_idx].instructions.push(instr);
                    }
                }
            }
        }
    }
    
    fn emit_terminator(&mut self, instr: LlvmInstr) {
        if let Some(func_name) = &self.current_function {
            if let Some(func_idx) = self.module.functions.iter().position(|f| &f.name == func_name) {
                if let Some(block_name) = &self.current_block {
                    if let Some(block_idx) = self.module.functions[func_idx].blocks.iter()
                        .position(|b| &b.name == block_name)
                    {
                        self.module.functions[func_idx].blocks[block_idx].terminator = Some(instr);
                    }
                }
            }
        }
    }
    
    /// Build the module and return it
    pub fn build(self) -> LlvmModule {
        self.module
    }
}

/// Generate LLVM IR text from a module
pub fn generate_ir(module: &LlvmModule) -> String {
    let mut output = String::new();
    
    // Module header
    writeln!(output, "; ModuleID = '{}'", module.name).unwrap();
    writeln!(output, "source_filename = \"bengal\"").unwrap();
    writeln!(output).unwrap();
    
    // Globals
    for (name, (ty, init)) in &module.globals {
        write!(output, "@{} = global {} ", name, ty.as_str()).unwrap();
        if let Some(init_val) = init {
            write!(output, "{}", init_val.as_str()).unwrap();
        } else {
            write!(output, "zeroinitializer").unwrap();
        }
        writeln!(output).unwrap();
    }
    
    if !module.globals.is_empty() {
        writeln!(output).unwrap();
    }
    
    // Functions
    for func in &module.functions {
        generate_function_ir(&mut output, func);
        writeln!(output).unwrap();
    }
    
    output
}

fn generate_function_ir(output: &mut String, func: &LlvmFunction) {
    // Function declaration
    let params: Vec<String> = func.param_types.iter()
        .enumerate()
        .map(|(i, ty)| format!("{} %{}", ty.as_str(), i))
        .collect();
    
    if func.is_external {
        writeln!(output, "declare {} @{}({})", 
            func.return_type.as_str(), 
            func.name, 
            params.join(", ")
        ).unwrap();
        return;
    }
    
    // Function definition
    writeln!(output, "define {} @{}({}) {{", 
        func.return_type.as_str(), 
        func.name, 
        params.join(", ")
    ).unwrap();
    
    // Blocks
    for block in &func.blocks {
        writeln!(output, "{}:", block.name).unwrap();
        
        // Instructions
        for instr in &block.instructions {
            generate_instruction(output, instr);
        }
        
        // Terminator
        if let Some(terminator) = &block.terminator {
            generate_instruction(output, terminator);
        }
        
        writeln!(output).unwrap();
    }
    
    writeln!(output, "}}").unwrap();
}

fn generate_instruction(output: &mut String, instr: &LlvmInstr) {
    match instr {
        LlvmInstr::Add(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = add {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::Sub(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = sub {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::Mul(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = mul {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::SDiv(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = sdiv {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::FAdd(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = fadd {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::FSub(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = fsub {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::FMul(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = fmul {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::FDiv(lhs, rhs, ty, reg) => {
            writeln!(output, "  %{} = fdiv {} {}, {}", reg, ty.as_str(), lhs.as_str(), rhs.as_str()).unwrap();
        }
        LlvmInstr::Alloca(ty, size, reg) => {
            write!(output, "  %{} = alloca {}", reg, ty.as_str()).unwrap();
            if let Some(s) = size {
                write!(output, ", {}", s).unwrap();
            }
            writeln!(output).unwrap();
        }
        LlvmInstr::Load(ty, ptr, reg) => {
            writeln!(output, "  %{} = load {}, ptr {}", reg, ty.as_str(), ptr.as_str()).unwrap();
        }
        LlvmInstr::Store(ty, value, ptr) => {
            writeln!(output, "  store {} {}, ptr {}", ty.as_str(), value.as_str(), ptr.as_str()).unwrap();
        }
        LlvmInstr::Ret(Some(value), ty) => {
            writeln!(output, "  ret {} {}", ty.as_str(), value.as_str()).unwrap();
        }
        LlvmInstr::Ret(None, _) => {
            writeln!(output, "  ret void").unwrap();
        }
        LlvmInstr::Br(target) => {
            writeln!(output, "  br label %{}", target).unwrap();
        }
        LlvmInstr::CondBr(cond, then_block, else_block) => {
            writeln!(output, "  br i1 {}, label %{}, label %{}", cond.as_str(), then_block, else_block).unwrap();
        }
        LlvmInstr::Call(ty, func_name, args, reg) => {
            let args_str: Vec<String> = args.iter().map(|a| a.as_str()).collect();
            writeln!(output, "  %{} = call {} @{}({})", reg, ty.as_str(), func_name, args_str.join(", ")).unwrap();
        }
        _ => {
            writeln!(output, "  ; TODO: {:?}", instr).unwrap();
        }
    }
}

fn get_result_register(output: &mut String) -> String {
    // Count existing % registers in the function to determine next register number
    let content = output.as_str();
    let count = content.matches('%').count();
    format!("%{}", count)
}

/// VERY experimental LLVM backend wrapper
pub struct LlvmBackend {
    module: Option<LlvmModule>,
}

impl LlvmBackend {
    pub fn new() -> Self {
        Self { module: None }
    }
    
    pub fn create_module(&mut self, name: &str) {
        self.module = Some(LlvmModule::new(name.to_string()));
    }
    
    pub fn get_module(&self) -> Option<&LlvmModule> {
        self.module.as_ref()
    }
    
    pub fn generate_ir(&self) -> Option<String> {
        self.module.as_ref().map(generate_ir)
    }
    
    pub fn do_nothing(&mut self) {
        // Still keeping the tradition alive
    }
    
    pub fn verify_nothing(&self) -> bool {
        self.module.is_some()
    }
    
    pub fn dump_module(&self) {
        if let Some(ir) = self.generate_ir() {
            println!("{}", ir);
        } else {
            println!("; No module loaded");
        }
    }
}

impl Default for LlvmBackend {
    fn default() -> Self {
        Self::new()
    }
}

/// LLVM IR Generator - main entry point for code generation
pub struct LlvmIrGenerator {
    builder: Option<LlvmBuilder>,
}

impl LlvmIrGenerator {
    pub fn new() -> Self {
        Self { builder: None }
    }
    
    pub fn start_module(&mut self, name: &str) {
        self.builder = Some(LlvmBuilder::new(name));
    }
    
    pub fn start_function(&mut self, name: &str, return_type: LlvmType, param_types: Vec<LlvmType>) {
        if let Some(ref mut builder) = self.builder {
            builder.begin_function(name, return_type, param_types);
        }
    }
    
    pub fn start_block(&mut self, name: &str) {
        if let Some(ref mut builder) = self.builder {
            builder.begin_block(name);
        }
    }
    
    pub fn end_block(&mut self) {
        if let Some(ref mut builder) = self.builder {
            builder.end_block();
        }
    }
    
    pub fn end_function(&mut self) {
        if let Some(ref mut builder) = self.builder {
            builder.end_function();
        }
    }
    
    // Arithmetic operations
    pub fn add(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.add(lhs, rhs, ty)
        } else {
            LlvmValue::IntConst(0)
        }
    }
    
    pub fn sub(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.sub(lhs, rhs, ty)
        } else {
            LlvmValue::IntConst(0)
        }
    }
    
    pub fn mul(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.mul(lhs, rhs, ty)
        } else {
            LlvmValue::IntConst(0)
        }
    }
    
    pub fn sdiv(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.sdiv(lhs, rhs, ty)
        } else {
            LlvmValue::IntConst(0)
        }
    }
    
    pub fn fadd(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.fadd(lhs, rhs, ty)
        } else {
            LlvmValue::FloatConst(0.0)
        }
    }
    
    pub fn fsub(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.fsub(lhs, rhs, ty)
        } else {
            LlvmValue::FloatConst(0.0)
        }
    }
    
    pub fn fmul(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.fmul(lhs, rhs, ty)
        } else {
            LlvmValue::FloatConst(0.0)
        }
    }
    
    pub fn fdiv(&mut self, lhs: LlvmValue, rhs: LlvmValue, ty: LlvmType) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.fdiv(lhs, rhs, ty)
        } else {
            LlvmValue::FloatConst(0.0)
        }
    }
    
    // Variables
    pub fn alloca(&mut self, ty: LlvmType, name: &str) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.alloca(ty, name)
        } else {
            LlvmValue::Alloca(name.to_string())
        }
    }
    
    pub fn load(&mut self, ty: LlvmType, ptr: LlvmValue) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.load(ty, ptr)
        } else {
            LlvmValue::Register(0)
        }
    }
    
    pub fn store(&mut self, ty: LlvmType, value: LlvmValue, ptr: LlvmValue) {
        if let Some(ref mut builder) = self.builder {
            builder.store(ty, value, ptr);
        }
    }
    
    // Control flow
    pub fn ret(&mut self, value: Option<LlvmValue>, ty: LlvmType) {
        if let Some(ref mut builder) = self.builder {
            builder.ret(value, ty);
        }
    }
    
    pub fn br(&mut self, target: &str) {
        if let Some(ref mut builder) = self.builder {
            builder.br(target);
        }
    }
    
    pub fn cond_br(&mut self, cond: LlvmValue, then_block: &str, else_block: &str) {
        if let Some(ref mut builder) = self.builder {
            builder.cond_br(cond, then_block, else_block);
        }
    }
    
    // Function calls
    pub fn call(&mut self, return_type: LlvmType, func_name: &str, args: Vec<LlvmValue>) -> LlvmValue {
        if let Some(ref mut builder) = self.builder {
            builder.call(return_type, func_name, args)
        } else {
            LlvmValue::Register(0)
        }
    }
    
    pub fn finish(&mut self) -> Option<String> {
        self.builder.take().map(|b| generate_ir(&b.build()))
    }
}

impl Default for LlvmIrGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_add_function() {
        let mut gen = LlvmIrGenerator::new();
        gen.start_module("test");
        gen.start_function("add", LlvmType::I32, vec![LlvmType::I32, LlvmType::I32]);
        gen.start_block("entry");
        
        // Load parameters (they're already in register form)
        let param0 = LlvmValue::Param(0);
        let param1 = LlvmValue::Param(1);
        
        // Add them
        let result = gen.add(param0, param1, LlvmType::I32);
        
        // Return
        gen.ret(Some(result), LlvmType::I32);
        gen.end_block();
        gen.end_function();
        
        let ir = gen.finish().unwrap();
        assert!(ir.contains("define i32 @add"));
        assert!(ir.contains("add i32"));
        assert!(ir.contains("ret i32"));
    }
    
    #[test]
    fn test_arithmetic_operations() {
        let mut gen = LlvmIrGenerator::new();
        gen.start_module("arith");
        gen.start_function("compute", LlvmType::I32, vec![LlvmType::I32]);
        gen.start_block("entry");
        
        let param = LlvmValue::Param(0);
        let two = LlvmValue::IntConst(2);
        let three = LlvmValue::IntConst(3);
        
        // param * 2
        let mul_result = gen.mul(param, two, LlvmType::I32);
        // (param * 2) + 3
        let add_result = gen.add(mul_result, three, LlvmType::I32);
        // (param * 2) + 3 - 1
        let sub_result = gen.sub(add_result, LlvmValue::IntConst(1), LlvmType::I32);
        
        gen.ret(Some(sub_result), LlvmType::I32);
        gen.end_block();
        gen.end_function();
        
        let ir = gen.finish().unwrap();
        assert!(ir.contains("mul i32"));
        assert!(ir.contains("add i32"));
        assert!(ir.contains("sub i32"));
    }
    
    #[test]
    fn test_float_operations() {
        let mut gen = LlvmIrGenerator::new();
        gen.start_module("float");
        gen.start_function("float_math", LlvmType::F64, vec![LlvmType::F64, LlvmType::F64]);
        gen.start_block("entry");
        
        let param0 = LlvmValue::Param(0);
        let param1 = LlvmValue::Param(1);
        
        let add_result = gen.fadd(param0.clone(), param1.clone(), LlvmType::F64);
        let mul_result = gen.fmul(add_result, param0, LlvmType::F64);
        let div_result = gen.fdiv(mul_result, param1, LlvmType::F64);
        
        gen.ret(Some(div_result), LlvmType::F64);
        gen.end_block();
        gen.end_function();
        
        let ir = gen.finish().unwrap();
        assert!(ir.contains("fadd double"));
        assert!(ir.contains("fmul double"));
        assert!(ir.contains("fdiv double"));
    }
    
    #[test]
    fn test_variables() {
        let mut gen = LlvmIrGenerator::new();
        gen.start_module("vars");
        gen.start_function("use_vars", LlvmType::I32, vec![]);
        gen.start_block("entry");
        
        // Allocate variable
        let var = gen.alloca(LlvmType::I32, "x");
        
        // Store value
        gen.store(LlvmType::I32, LlvmValue::IntConst(42), var.clone());
        
        // Load value
        let loaded = gen.load(LlvmType::I32, var);
        
        gen.ret(Some(loaded), LlvmType::I32);
        gen.end_block();
        gen.end_function();
        
        let ir = gen.finish().unwrap();
        assert!(ir.contains("alloca i32"));
        assert!(ir.contains("store i32"));
        assert!(ir.contains("load i32"));
    }
    
    #[test]
    fn test_complex_function() {
        let mut gen = LlvmIrGenerator::new();
        gen.start_module("complex");
        
        // Function: compute(a: i32, b: i32) -> i32
        // return (a + b) * (a - b) / 2
        gen.start_function("compute", LlvmType::I32, vec![LlvmType::I32, LlvmType::I32]);
        gen.start_block("entry");
        
        let a = LlvmValue::Param(0);
        let b = LlvmValue::Param(1);
        
        let sum = gen.add(a.clone(), b.clone(), LlvmType::I32);
        let diff = gen.sub(a, b, LlvmType::I32);
        let product = gen.mul(sum, diff, LlvmType::I32);
        let result = gen.sdiv(product, LlvmValue::IntConst(2), LlvmType::I32);
        
        gen.ret(Some(result), LlvmType::I32);
        gen.end_block();
        gen.end_function();
        
        let ir = gen.finish().unwrap();
        println!("{}", ir);
        
        assert!(ir.contains("define i32 @compute"));
        assert!(ir.contains("add i32"));
        assert!(ir.contains("sub i32"));
        assert!(ir.contains("mul i32"));
        assert!(ir.contains("sdiv i32"));
        assert!(ir.contains("ret i32"));
    }
}

//! High-Level Intermediate Representation (HLIR)
//! 
//! This is a simplified, uniform IR that sits between the AST and backend code generators.
//! It can be lowered to both LLVM IR and Sparkler bytecode.

use std::fmt::Write;

/// HLIR Type system
#[derive(Debug, Clone, PartialEq)]
pub enum HlirType {
    Void,
    Bool,
    I8,
    I32,
    I64,
    F32,
    F64,
    String,
    Array(Box<HlirType>),
    Pointer(Box<HlirType>),
    Function(Vec<HlirType>, Box<HlirType>),  // params, return
    Unknown,
}

#[cfg(feature = "llvm")]
impl HlirType {
    pub fn to_llvm_type(&self) -> crate::llvm::LlvmType {
        match self {
            HlirType::Void => crate::llvm::LlvmType::Void,
            HlirType::Bool => crate::llvm::LlvmType::I1,
            HlirType::I8 | HlirType::I32 => crate::llvm::LlvmType::I32,
            HlirType::I64 => crate::llvm::LlvmType::I64,
            HlirType::F32 => crate::llvm::LlvmType::F32,
            HlirType::F64 => crate::llvm::LlvmType::F64,
            HlirType::String => crate::llvm::LlvmType::Pointer(Box::new(crate::llvm::LlvmType::I32)),
            HlirType::Array(inner) => crate::llvm::LlvmType::Pointer(Box::new(inner.to_llvm_type())),
            HlirType::Pointer(inner) => crate::llvm::LlvmType::Pointer(Box::new(inner.to_llvm_type())),
            HlirType::Function(_, _) => crate::llvm::LlvmType::Pointer(Box::new(crate::llvm::LlvmType::Void)),
            HlirType::Unknown => crate::llvm::LlvmType::I32,
        }
    }
}

/// HLIR Value - represents values in the IR
#[derive(Debug, Clone)]
pub enum HlirValue {
    /// Constant integer
    IntConst(i64),
    /// Constant float
    FloatConst(f64),
    /// Constant boolean
    BoolConst(bool),
    /// Constant string
    StringConst(String),
    /// Local variable reference
    Local(String),
    /// Function parameter
    Param(usize),
    /// Function reference
    Function(String),
    /// Temporary value (SSA-style)
    Temp(usize),
}

/// HLIR Binary Operations
#[derive(Debug, Clone, Copy)]
pub enum HlirBinOp {
    Add, Sub, Mul, SDiv, UDiv, SRem, URem,
    FAdd, FSub, FMul, FDiv,
    And, Or, Xor,
    Eq, Ne,
    Sgt, Sge, Slt, Sle,
    Ugt, Uge, Ult, Ule,
    Oeq, One, Ogt, Oge, Olt, Ole,
    Shl, LShr, AShr,
}

/// HLIR Unary Operations
#[derive(Debug, Clone, Copy)]
pub enum HlirUnaryOp {
    Neg,
    Not,
    LNot,  // Logical not
}

/// HLIR Instructions
#[derive(Debug, Clone)]
pub enum HlirInstr {
    /// Binary operation: dest = op(lhs, rhs)
    BinOp {
        op: HlirBinOp,
        lhs: HlirValue,
        rhs: HlirValue,
        dest: usize,
        ty: HlirType,
    },
    
    /// Unary operation: dest = op(value)
    UnaryOp {
        op: HlirUnaryOp,
        value: HlirValue,
        dest: usize,
        ty: HlirType,
    },
    
    /// Load from memory: dest = *ptr
    Load {
        ptr: HlirValue,
        dest: usize,
        ty: HlirType,
    },
    
    /// Store to memory: *ptr = value
    Store {
        value: HlirValue,
        ptr: HlirValue,
        ty: HlirType,
    },
    
    /// Stack allocation: dest = alloca ty
    Alloca {
        ty: HlirType,
        dest: usize,
        name: String,
    },
    
    /// Function call: dest = call func(args) (dest is None if return value is unused)
    Call {
        func: HlirValue,
        args: Vec<HlirValue>,
        dest: Option<usize>,
        return_ty: HlirType,
        arg_types: Vec<HlirType>,  // Argument types for mangling
    },

    /// String concatenation: dest = concat(values[0], values[1], ...)
    /// Optimized for interpolated strings with multiple parts
    StringConcat {
        values: Vec<HlirValue>,
        dest: usize,
    },
    
    /// Return from function
    Return {
        value: Option<HlirValue>,
        ty: HlirType,
    },
    
    /// Unconditional branch
    Br {
        target: String,
    },
    
    /// Conditional branch
    CondBr {
        cond: HlirValue,
        then_block: String,
        else_block: String,
    },
    
    /// Phi node for SSA
    Phi {
        ty: HlirType,
        sources: Vec<(HlirValue, String)>,  // (value, block_name)
        dest: usize,
    },
    
    /// Cast/convert type
    Cast {
        value: HlirValue,
        from_ty: HlirType,
        to_ty: HlirType,
        dest: usize,
        kind: HlirCastKind,
    },
    
    /// Get element pointer
    GetElementPtr {
        base: HlirValue,
        indices: Vec<HlirValue>,
        dest: usize,
        ty: HlirType,
    },
    
    /// Compare values
    Cmp {
        op: HlirBinOp,
        lhs: HlirValue,
        rhs: HlirValue,
        dest: usize,
        ty: HlirType,
    },
    
    /// Select (ternary): dest = cond ? then_val : else_val
    Select {
        cond: HlirValue,
        then_val: HlirValue,
        else_val: HlirValue,
        dest: usize,
        ty: HlirType,
    },

    /// Exception handling: start of try block
    TryStart {
        catch_block: String,
        catch_reg: usize,
    },

    /// Exception handling: end of try block
    TryEnd,

    /// Throw exception
    Throw {
        value: HlirValue,
    },

    /// Set property: object.field = value
    /// Used for field assignments in constructors and methods
    SetProperty {
        object: HlirValue,
        field_name: String,
        value: HlirValue,
    },

    /// Get property: dest = object.field
    /// Used for field access in methods
    GetProperty {
        object: HlirValue,
        field_name: String,
        dest: usize,
    },
}

/// Cast kinds
#[derive(Debug, Clone, Copy)]
pub enum HlirCastKind {
    Trunc,    // Truncate to smaller type
    ZExt,     // Zero extend to larger type
    SExt,     // Sign extend to larger type
    FpToSi,   // Float to signed int
    FpToUi,   // Float to unsigned int
    SiToFp,   // Signed int to float
    UiToFp,   // Unsigned int to float
    BitCast,  // Bitwise cast
    PtrToInt, // Pointer to int
    IntToPtr, // Int to pointer
}

/// HLIR Basic Block
#[derive(Debug, Clone)]
pub struct HlirBasicBlock {
    pub name: String,
    pub instructions: Vec<HlirInstr>,
    pub terminator: Option<HlirInstr>,
}

impl HlirBasicBlock {
    pub fn new(name: String) -> Self {
        Self {
            name,
            instructions: Vec::new(),
            terminator: None,
        }
    }
}

/// HLIR Function
#[derive(Debug, Clone)]
pub struct HlirFunction {
    pub name: String,
    pub params: Vec<(String, HlirType)>,  // (name, type)
    pub return_type: HlirType,
    pub blocks: Vec<HlirBasicBlock>,
    pub is_external: bool,
    pub is_variadic: bool,
}

impl HlirFunction {
    pub fn new(name: String, params: Vec<(String, HlirType)>, return_type: HlirType) -> Self {
        Self {
            name,
            params,
            return_type,
            blocks: Vec::new(),
            is_external: false,
            is_variadic: false,
        }
    }
    
    pub fn external(name: String, params: Vec<(String, HlirType)>, return_type: HlirType) -> Self {
        let mut f = Self::new(name, params, return_type);
        f.is_external = true;
        f
    }
}

/// HLIR Global variable
#[derive(Debug, Clone)]
pub struct HlirGlobal {
    pub name: String,
    pub ty: HlirType,
    pub initializer: Option<HlirValue>,
    pub is_constant: bool,
}

/// HLIR Module - top-level container
#[derive(Debug, Clone)]
pub struct HlirModule {
    pub name: String,
    pub functions: Vec<HlirFunction>,
    pub globals: Vec<HlirGlobal>,
    pub classes: Vec<HlirClass>,
}

/// HLIR Class information
#[derive(Debug, Clone)]
pub struct HlirClass {
    pub name: String,
    pub fields: Vec<String>,
    pub private_fields: Vec<String>,
    pub methods: Vec<String>,
    pub is_native: bool,
    pub is_interface: bool,
}

impl HlirModule {
    pub fn new(name: String) -> Self {
        Self {
            name,
            functions: Vec::new(),
            globals: Vec::new(),
            classes: Vec::new(),
        }
    }

    pub fn add_function(&mut self, func: HlirFunction) {
        self.functions.push(func);
    }

    pub fn add_global(&mut self, global: HlirGlobal) {
        self.globals.push(global);
    }

    pub fn add_class(&mut self, class: HlirClass) {
        self.classes.push(class);
    }
}

/// HLIR Builder - helps construct HLIR incrementally
#[derive(Clone)]
pub struct HlirBuilder {
    module: HlirModule,
    current_function: Option<String>,
    current_block: Option<String>,
    next_temp: usize,
    variables: std::collections::HashMap<String, usize>,
}

impl HlirBuilder {
    pub fn new(module_name: &str) -> Self {
        Self {
            module: HlirModule::new(module_name.to_string()),
            current_function: None,
            current_block: None,
            next_temp: 0,
            variables: std::collections::HashMap::new(),
        }
    }

    pub fn add_class(&mut self, class: HlirClass) {
        self.module.add_class(class);
    }
    
    /// Start building a function
    pub fn begin_function(&mut self, name: &str, params: Vec<(String, HlirType)>, return_type: HlirType) {
        let func = HlirFunction::new(name.to_string(), params, return_type);
        self.module.add_function(func);
        self.current_function = Some(name.to_string());
        self.next_temp = 0;
        self.variables.clear();
        
        // Register parameters as variables
        for (i, (param_name, _)) in self.module.functions.last().unwrap().params.iter().enumerate() {
            self.variables.insert(param_name.clone(), i);
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
            let block = HlirBasicBlock::new(name.to_string());
            self.module.functions[func_idx].blocks.push(block);
        }
    }
    
    /// End the current block
    pub fn end_block(&mut self) {
        self.current_block = None;
    }
    
    /// Allocate a new temporary
    pub fn new_temp(&mut self) -> usize {
        let temp = self.next_temp;
        self.next_temp += 1;
        temp
    }

    /// Get the type of a value
    pub fn get_value_type(&self, value: &HlirValue) -> HlirType {
        match value {
            HlirValue::IntConst(_) => HlirType::I32,
            HlirValue::FloatConst(_) => HlirType::F64,
            HlirValue::BoolConst(_) => HlirType::Bool,
            HlirValue::StringConst(_) => HlirType::String,
            HlirValue::Temp(_) => HlirType::Unknown,
            HlirValue::Local(_) => HlirType::Unknown,
            HlirValue::Param(_) => HlirType::Unknown,
            HlirValue::Function(_) => HlirType::Unknown,
        }
    }

    /// Generate a binary operation
    pub fn bin_op(&mut self, op: HlirBinOp, lhs: HlirValue, rhs: HlirValue, ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::BinOp { op, lhs, rhs, dest, ty: ty.clone() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate a unary operation
    pub fn unary_op(&mut self, op: HlirUnaryOp, value: HlirValue, ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::UnaryOp { op, value, dest, ty: ty.clone() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate an alloca
    pub fn alloca(&mut self, ty: HlirType, name: &str) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::Alloca { ty: ty.clone(), dest, name: name.to_string() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate a load
    pub fn load(&mut self, ptr: HlirValue, ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::Load { ptr, dest, ty: ty.clone() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate a store
    pub fn store(&mut self, value: HlirValue, ptr: HlirValue, ty: HlirType) {
        let instr = HlirInstr::Store { value, ptr, ty: ty.clone() };
        self.emit(instr);
    }

    /// Generate a set property instruction
    pub fn set_property(&mut self, object: HlirValue, field_name: &str, value: HlirValue) {
        let instr = HlirInstr::SetProperty {
            object,
            field_name: field_name.to_string(),
            value
        };
        self.emit(instr);
    }

    /// Generate a get property instruction
    pub fn get_property(&mut self, object: HlirValue, field_name: &str) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::GetProperty {
            object,
            field_name: field_name.to_string(),
            dest
        };
        self.emit(instr);
        HlirValue::Temp(dest)
    }

    /// Generate a call
    pub fn call(&mut self, func: HlirValue, args: Vec<HlirValue>, return_ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let arg_types: Vec<HlirType> = args.iter().map(|a| self.get_value_type(a)).collect();
        let instr = HlirInstr::Call { func, args, dest: Some(dest), return_ty: return_ty.clone(), arg_types };
        self.emit(instr);
        HlirValue::Temp(dest)
    }

    /// Generate a call, discarding the return value
    pub fn call_discard(&mut self, func: HlirValue, args: Vec<HlirValue>, return_ty: HlirType) {
        let arg_types: Vec<HlirType> = args.iter().map(|a| self.get_value_type(a)).collect();
        let instr = HlirInstr::Call { func, args, dest: None, return_ty: return_ty.clone(), arg_types };
        self.emit(instr);
    }

    /// Generate a string concatenation with multiple operands (optimized for interpolated strings)
    pub fn string_concat(&mut self, values: Vec<HlirValue>) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::StringConcat { values, dest };
        self.emit(instr);
        HlirValue::Temp(dest)
    }

    /// Generate a return
    pub fn ret(&mut self, value: Option<HlirValue>, ty: HlirType) {
        let instr = HlirInstr::Return { value, ty: ty.clone() };
        self.emit_terminator(instr);
    }
    
    /// Generate a branch
    pub fn br(&mut self, target: &str) {
        let instr = HlirInstr::Br { target: target.to_string() };
        self.emit_terminator(instr);
    }
    
    /// Generate a conditional branch
    pub fn cond_br(&mut self, cond: HlirValue, then_block: &str, else_block: &str) {
        let instr = HlirInstr::CondBr { cond, then_block: then_block.to_string(), else_block: else_block.to_string() };
        self.emit_terminator(instr);
    }
    
    /// Generate a cast
    pub fn cast(&mut self, value: HlirValue, from_ty: HlirType, to_ty: HlirType, kind: HlirCastKind) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::Cast { value, from_ty, to_ty, dest, kind };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate a comparison
    pub fn cmp(&mut self, op: HlirBinOp, lhs: HlirValue, rhs: HlirValue, ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::Cmp { op, lhs, rhs, dest, ty: ty.clone() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }
    
    /// Generate a select
    pub fn select(&mut self, cond: HlirValue, then_val: HlirValue, else_val: HlirValue, ty: HlirType) -> HlirValue {
        let dest = self.new_temp();
        let instr = HlirInstr::Select { cond, then_val, else_val, dest, ty: ty.clone() };
        self.emit(instr);
        HlirValue::Temp(dest)
    }

    /// Generate a try_start (beginning of try block)
    pub fn try_start(&mut self, catch_block: &str, catch_reg: usize) {
        let instr = HlirInstr::TryStart { 
            catch_block: catch_block.to_string(), 
            catch_reg 
        };
        self.emit(instr);
    }

    /// Generate a try_end (end of try block)
    pub fn try_end(&mut self) {
        let instr = HlirInstr::TryEnd;
        self.emit(instr);
    }

    /// Generate a throw
    pub fn throw(&mut self, value: HlirValue) {
        let instr = HlirInstr::Throw { value };
        self.emit(instr);
    }
    
    fn emit(&mut self, instr: HlirInstr) {
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
    
    fn emit_terminator(&mut self, instr: HlirInstr) {
        if let Some(func_name) = &self.current_function {
            if let Some(func_idx) = self.module.functions.iter().position(|f| &f.name == func_name) {
                if let Some(block_name) = &self.current_block {
                    if let Some(block_idx) = self.module.functions[func_idx].blocks.iter()
                        .position(|b| &b.name == block_name)
                    {
                        // Only set terminator if one doesn't already exist
                        if self.module.functions[func_idx].blocks[block_idx].terminator.is_none() {
                            self.module.functions[func_idx].blocks[block_idx].terminator = Some(instr);
                        }
                    }
                }
            }
        }
    }
    
    /// Build and return the module
    pub fn build(self) -> HlirModule {
        self.module
    }
}

/// Lower HLIR to LLVM IR
#[cfg(feature = "llvm")]
pub fn lower_hlir_to_llvm(hlir: &HlirModule) -> crate::llvm::LlvmModule {
    use crate::llvm::{LlvmModule, LlvmFunction, LlvmBasicBlock, LlvmValue as Lv, LlvmType as Lt, LlvmInstr, LlvmCompareOp};

    let mut llvm_module = LlvmModule::new(hlir.name.clone());
    
    for hlir_func in &hlir.functions {
        let param_types: Vec<Lt> = hlir_func.params.iter()
            .map(|(_, ty)| ty.to_llvm_type())
            .collect();
        
        let mut llvm_func = if hlir_func.is_external {
            LlvmFunction::external(
                hlir_func.name.clone(),
                hlir_func.return_type.to_llvm_type(),
                param_types,
            )
        } else {
            LlvmFunction::new(
                hlir_func.name.clone(),
                hlir_func.return_type.to_llvm_type(),
                param_types,
            )
        };
        
        // Lower blocks
        for hlir_block in &hlir_func.blocks {
            let mut llvm_block = LlvmBasicBlock::new(hlir_block.name.clone());
            
            // Lower instructions
            for hlir_instr in &hlir_block.instructions {
                lower_instr_to_llvm(hlir_instr, &mut llvm_block.instructions);
            }
            
            // Lower terminator
            if let Some(term) = &hlir_block.terminator {
                if let Some(llvm_term) = lower_single_instr_to_llvm(term) {
                    llvm_block.terminator = Some(llvm_term);
                }
            }
            
            llvm_func.blocks.push(llvm_block);
        }
        
        llvm_module.add_function(llvm_func);
    }

    llvm_module
}

#[cfg(feature = "llvm")]
fn lower_instr_to_llvm(hlir_instr: &HlirInstr, llvm_instrs: &mut Vec<crate::llvm::LlvmInstr>) {
    if let Some(instr) = lower_single_instr_to_llvm(hlir_instr) {
        llvm_instrs.push(instr);
    }
}

#[cfg(feature = "llvm")]
fn lower_single_instr_to_llvm(hlir_instr: &HlirInstr) -> Option<crate::llvm::LlvmInstr> {
    use crate::llvm::{LlvmValue as Lv, LlvmType as Lt, LlvmInstr, LlvmCompareOp};

    let hlir_to_llvm_value = |v: &HlirValue| -> Lv {
        match v {
            HlirValue::IntConst(n) => Lv::IntConst(*n),
            HlirValue::FloatConst(n) => Lv::FloatConst(*n),
            HlirValue::BoolConst(b) => Lv::BoolConst(*b),
            HlirValue::StringConst(s) => Lv::StringConst(s.clone()),
            HlirValue::Local(name) => Lv::Alloca(name.clone()),
            HlirValue::Param(n) => Lv::Param(*n as u32),
            HlirValue::Function(name) => Lv::Global(name.clone()),
            HlirValue::Temp(n) => Lv::Register(*n as u32),
        }
    };
    
    let hlir_to_llvm_cmp = |op: &HlirBinOp| -> LlvmCompareOp {
        match op {
            HlirBinOp::Eq => LlvmCompareOp::Eq,
            HlirBinOp::Ne => LlvmCompareOp::Ne,
            HlirBinOp::Sgt => LlvmCompareOp::Sgt,
            HlirBinOp::Sge => LlvmCompareOp::Sge,
            HlirBinOp::Slt => LlvmCompareOp::Slt,
            HlirBinOp::Sle => LlvmCompareOp::Sle,
            HlirBinOp::Ugt => LlvmCompareOp::Ugt,
            HlirBinOp::Uge => LlvmCompareOp::Uge,
            HlirBinOp::Ult => LlvmCompareOp::Ult,
            HlirBinOp::Ule => LlvmCompareOp::Ule,
            HlirBinOp::Oeq => LlvmCompareOp::Oeq,
            HlirBinOp::One => LlvmCompareOp::One,
            HlirBinOp::Ogt => LlvmCompareOp::Ogt,
            HlirBinOp::Oge => LlvmCompareOp::Oge,
            HlirBinOp::Olt => LlvmCompareOp::Olt,
            HlirBinOp::Ole => LlvmCompareOp::Ole,
            _ => LlvmCompareOp::Eq,
        }
    };

    match hlir_instr {
        HlirInstr::BinOp { op, lhs, rhs, dest, ty } => {
            let llvm_ty = ty.to_llvm_type();
            let llvm_lhs = hlir_to_llvm_value(lhs);
            let llvm_rhs = hlir_to_llvm_value(rhs);
            let reg = *dest as u32;

            match op {
                HlirBinOp::Add => Some(LlvmInstr::Add(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::Sub => Some(LlvmInstr::Sub(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::Mul => Some(LlvmInstr::Mul(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::SDiv => Some(LlvmInstr::SDiv(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::UDiv => Some(LlvmInstr::UDiv(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::FAdd => Some(LlvmInstr::FAdd(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::FSub => Some(LlvmInstr::FSub(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::FMul => Some(LlvmInstr::FMul(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::FDiv => Some(LlvmInstr::FDiv(llvm_lhs, llvm_rhs, llvm_ty, reg)),
                HlirBinOp::And => Some(LlvmInstr::And(llvm_lhs, llvm_rhs)),
                HlirBinOp::Or => Some(LlvmInstr::Or(llvm_lhs, llvm_rhs)),
                HlirBinOp::Xor => Some(LlvmInstr::Xor(llvm_lhs, llvm_rhs)),
                _ => None,
            }
        }
        HlirInstr::UnaryOp { op, value, dest, ty } => {
            let llvm_ty = ty.to_llvm_type();
            let llvm_value = hlir_to_llvm_value(value);
            let reg = *dest as u32;
            
            match op {
                HlirUnaryOp::Not => Some(LlvmInstr::Not(llvm_value)),
                _ => None,
            }
        }
        HlirInstr::Alloca { ty, dest, name: _ } => {
            let llvm_ty = ty.to_llvm_type();
            let reg = *dest as u32;
            Some(LlvmInstr::Alloca(llvm_ty, None, reg))
        }
        HlirInstr::Load { ptr, dest, ty } => {
            let llvm_ty = ty.to_llvm_type();
            let llvm_ptr = hlir_to_llvm_value(ptr);
            let reg = *dest as u32;
            Some(LlvmInstr::Load(llvm_ty, llvm_ptr, reg))
        }
        HlirInstr::Store { value, ptr, ty } => {
            let llvm_ty = ty.to_llvm_type();
            let llvm_value = hlir_to_llvm_value(value);
            let llvm_ptr = hlir_to_llvm_value(ptr);
            Some(LlvmInstr::Store(llvm_ty, llvm_value, llvm_ptr))
        }
        HlirInstr::Return { value, ty } => {
            let llvm_ty = ty.to_llvm_type();
            let llvm_value = value.as_ref().map(hlir_to_llvm_value);
            Some(LlvmInstr::Ret(llvm_value, llvm_ty))
        }
        HlirInstr::Br { target } => {
            Some(LlvmInstr::Br(target.clone()))
        }
        HlirInstr::CondBr { cond, then_block, else_block } => {
            let llvm_cond = hlir_to_llvm_value(cond);
            Some(LlvmInstr::CondBr(llvm_cond, then_block.clone(), else_block.clone()))
        }
        HlirInstr::Call { func, args, dest, return_ty, arg_types: _ } => {
            // Only emit LLVM IR if the return value is used
            if let Some(dest) = dest {
                let llvm_return_ty = return_ty.to_llvm_type();
                let llvm_func = hlir_to_llvm_value(func);
                let llvm_args: Vec<Lv> = args.iter().map(hlir_to_llvm_value).collect();
                let reg = *dest as u32;

                if let Lv::Global(name) = llvm_func {
                    Some(LlvmInstr::Call(llvm_return_ty, name, llvm_args, reg))
                } else {
                    None
                }
            } else {
                None  // Call without destination - skip LLVM IR generation
            }
        }
        HlirInstr::Cmp { op, lhs, rhs, dest: _, ty } => {
            let llvm_op = hlir_to_llvm_cmp(op);
            let llvm_lhs = hlir_to_llvm_value(lhs);
            let llvm_rhs = hlir_to_llvm_value(rhs);

            if matches!(ty, HlirType::F32 | HlirType::F64) {
                Some(LlvmInstr::FCmp(llvm_op, llvm_lhs, llvm_rhs))
            } else {
                Some(LlvmInstr::ICmp(llvm_op, llvm_lhs, llvm_rhs))
            }
        }
        _ => None,
    }
}

/// Generate LLVM IR text from HLIR
#[cfg(feature = "llvm")]
pub fn generate_llvm_ir_from_hlir(hlir: &HlirModule) -> String {
    let llvm_module = lower_hlir_to_llvm(hlir);
    crate::llvm::generate_ir(&llvm_module)
}

/// Generate HLIR text representation (for debugging)
pub fn generate_hlir_text(hlir: &HlirModule) -> String {
    let mut output = String::new();
    
    writeln!(output, "; HLIR Module: {}", hlir.name).unwrap();
    writeln!(output).unwrap();
    
    // Globals
    for global in &hlir.globals {
        let const_str = if global.is_constant { "const " } else { "" };
        writeln!(output, "{}global {} @{};", const_str, format!("{:?}", global.ty), global.name).unwrap();
    }
    
    if !hlir.globals.is_empty() {
        writeln!(output).unwrap();
    }
    
    // Functions
    for func in &hlir.functions {
        let params: Vec<String> = func.params.iter()
            .map(|(name, ty)| format!("{}: {:?}", name, ty))
            .collect();
        
        let ext_str = if func.is_external { "extern " } else { "" };
        writeln!(output, "{}fn {}({}) -> {:?}", ext_str, func.name, params.join(", "), func.return_type).unwrap();
        writeln!(output, "{{").unwrap();
        
        for block in &func.blocks {
            writeln!(output, "  {}:", block.name).unwrap();
            
            for instr in &block.instructions {
                writeln!(output, "    {:?}", instr).unwrap();
            }
            
            if let Some(term) = &block.terminator {
                writeln!(output, "    {:?}", term).unwrap();
            }
            
            writeln!(output).unwrap();
        }
        
        writeln!(output, "}}").unwrap();
        writeln!(output).unwrap();
    }
    
    output
}

#[cfg(test)]
#[cfg(feature = "llvm")]
mod tests {
    use super::*;

    #[test]
    fn test_simple_function() {
        let mut builder = HlirBuilder::new("test");
        
        builder.begin_function("add", vec![
            ("a".to_string(), HlirType::I32),
            ("b".to_string(), HlirType::I32),
        ], HlirType::I32);
        builder.begin_block("entry");
        
        let a = HlirValue::Param(0);
        let b = HlirValue::Param(1);
        
        let result = builder.bin_op(HlirBinOp::Add, a, b, HlirType::I32);
        builder.ret(Some(result), HlirType::I32);
        
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let ir = generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("define i32 @add"));
        assert!(ir.contains("add i32"));
        assert!(ir.contains("ret i32"));
    }
    
    #[test]
    fn test_arithmetic_expression() {
        let mut builder = HlirBuilder::new("arith");
        
        builder.begin_function("compute", vec![
            ("x".to_string(), HlirType::I32),
        ], HlirType::I32);
        builder.begin_block("entry");
        
        let x = HlirValue::Param(0);
        let two = HlirValue::IntConst(2);
        let three = HlirValue::IntConst(3);
        
        // (x * 2) + 3
        let mul = builder.bin_op(HlirBinOp::Mul, x.clone(), two, HlirType::I32);
        let add = builder.bin_op(HlirBinOp::Add, mul, three, HlirType::I32);
        
        builder.ret(Some(add), HlirType::I32);
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let ir = generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("mul i32"));
        assert!(ir.contains("add i32"));
    }
    
    #[test]
    fn test_variables() {
        let mut builder = HlirBuilder::new("vars");
        
        builder.begin_function("use_var", vec![], HlirType::I32);
        builder.begin_block("entry");
        
        // var x = alloca i32
        let x_ptr = builder.alloca(HlirType::I32, "x");
        
        // store 42, x
        builder.store(HlirValue::IntConst(42), x_ptr.clone(), HlirType::I32);
        
        // load x
        let loaded = builder.load(x_ptr, HlirType::I32);
        
        builder.ret(Some(loaded), HlirType::I32);
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let ir = generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("alloca i32"));
        assert!(ir.contains("store i32"));
        assert!(ir.contains("load i32"));
    }
    
    #[test]
    fn test_hlir_text_output() {
        let mut builder = HlirBuilder::new("debug");
        
        builder.begin_function("test", vec![
            ("a".to_string(), HlirType::I32),
        ], HlirType::I32);
        builder.begin_block("entry");
        
        let a = HlirValue::Param(0);
        let result = builder.bin_op(HlirBinOp::Add, a.clone(), a, HlirType::I32);
        builder.ret(Some(result), HlirType::I32);
        
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let text = generate_hlir_text(&hlir);
        
        assert!(text.contains("fn test"));
        assert!(text.contains("entry:"));
        assert!(text.contains("BinOp"));
    }
}

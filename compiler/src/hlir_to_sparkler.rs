//! HLIR to Sparkler Bytecode Compiler
//! 
//! This module compiles HLIR (High-Level IR) to Sparkler bytecode.

use crate::hlir::{HlirModule, HlirFunction, HlirBasicBlock, HlirInstr, HlirValue, HlirBinOp, HlirUnaryOp};
use sparkler::opcodes::Opcode;

/// Compiled bytecode with metadata
#[derive(Debug, Clone)]
pub struct CompiledBytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub max_registers: usize,
}

/// HLIR to Sparkler bytecode compiler
pub struct HlirToSparkler {
    bytecode: Vec<u8>,
    strings: Vec<String>,
    /// Map from HLIR temp/register to Sparkler register
    reg_map: std::collections::HashMap<usize, u8>,
    /// Next available Sparkler register
    next_sparkler_reg: u16,
    /// Current max register used
    max_reg: u16,
    /// Variable name to register mapping
    var_map: std::collections::HashMap<String, u8>,
    /// Block labels to bytecode offsets
    block_offsets: std::collections::HashMap<String, usize>,
    /// Forward jumps that need patching: (offset_in_bytecode, target_label)
    pending_jumps: Vec<(usize, String)>,
}

impl HlirToSparkler {
    pub fn new() -> Self {
        Self {
            bytecode: Vec::new(),
            strings: Vec::new(),
            reg_map: std::collections::HashMap::new(),
            next_sparkler_reg: 1, // R0 is for return value
            max_reg: 0,
            var_map: std::collections::HashMap::new(),
            block_offsets: std::collections::HashMap::new(),
            pending_jumps: Vec::new(),
        }
    }
    
    /// Allocate a new Sparkler register
    fn alloc_reg(&mut self) -> u8 {
        // Wrap around to reuse registers when we run out
        let reg = if self.next_sparkler_reg > 255 {
            // Reset and reuse - this is a simple approach
            self.next_sparkler_reg = 2; // Start after R0 and R1
            2
        } else {
            let reg = self.next_sparkler_reg as u8;
            self.next_sparkler_reg += 1;
            if reg > self.max_reg as u8 {
                self.max_reg = reg as u16;
            }
            reg
        };
        reg
    }

    /// Get or create a Sparkler register for an HLIR temp
    fn get_reg_for_temp(&mut self, temp: usize) -> u8 {
        if let Some(&reg) = self.reg_map.get(&temp) {
            reg
        } else {
            let reg = self.alloc_reg();
            self.reg_map.insert(temp, reg);
            reg
        }
    }
    
    /// Add a string to the string table and return its index
    fn add_string(&mut self, s: String) -> usize {
        if let Some(idx) = self.strings.iter().position(|existing| *existing == s) {
            idx
        } else {
            let idx = self.strings.len();
            self.strings.push(s);
            idx
        }
    }
    
    /// Emit a single byte
    fn emit(&mut self, byte: u8) {
        self.bytecode.push(byte);
    }
    
    /// Emit multiple bytes
    fn emit_bytes(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.bytecode.push(b);
        }
    }
    
    /// Emit an opcode
    fn emit_opcode(&mut self, opcode: Opcode) {
        self.emit(opcode as u8);
    }
    
    /// Emit a u16 value (little-endian)
    fn emit_u16(&mut self, value: u16) {
        self.emit((value & 0xFF) as u8);
        self.emit(((value >> 8) & 0xFF) as u8);
    }
    
    /// Emit an i64 value (little-endian)
    fn emit_i64(&mut self, value: i64) {
        for i in 0..8 {
            self.emit(((value >> (i * 8)) & 0xFF) as u8);
        }
    }
    
    /// Emit an f64 value (little-endian)
    fn emit_f64(&mut self, value: f64) {
        let bits = value.to_bits();
        for i in 0..8 {
            self.emit(((bits >> (i * 8)) & 0xFF) as u8);
        }
    }
    
    /// Get current bytecode offset
    fn current_offset(&self) -> usize {
        self.bytecode.len()
    }
    
    /// Patch a jump target at the given offset
    fn patch_jump(&mut self, offset: usize, target: usize) {
        let relative = target as i32 - offset as i32;
        self.bytecode[offset] = ((relative >> 8) & 0xFF) as u8;
        self.bytecode[offset + 1] = (relative & 0xFF) as u8;
    }
    
    /// Compile an HLIR module to Sparkler bytecode
    pub fn compile_module(&mut self, hlir: &HlirModule) -> CompiledBytecode {
        // First pass: collect function info and block offsets
        for func in &hlir.functions {
            let mut offset = self.current_offset();
            for block in &func.blocks {
                self.block_offsets.insert(format!("{}:{}", func.name, block.name), offset);
                // Estimate block size for forward references
                offset += block.instructions.len() * 4 + 10;
            }
        }
        
        // Second pass: compile functions
        for func in &hlir.functions {
            self.compile_function(func);
        }
        
        CompiledBytecode {
            data: self.bytecode.clone(),
            strings: self.strings.clone(),
            max_registers: self.max_reg as usize + 1,
        }
    }
    
    /// Compile a single function
    fn compile_function(&mut self, func: &HlirFunction) {
        // Reset state for new function
        self.reg_map.clear();
        self.var_map.clear();
        self.next_sparkler_reg = 1;
        self.max_reg = 0;

        // Allocate registers for parameters (R1, R2, ...)
        for (i, (name, _)) in func.params.iter().enumerate() {
            let reg = (i + 1) as u8;
            self.var_map.insert(name.clone(), reg);
            if reg as u16 > self.max_reg {
                self.max_reg = reg as u16;
            }
        }
        
        // Compile blocks
        for block in &func.blocks {
            self.compile_block(block, func);
        }
        
        // Patch pending jumps
        let pending = std::mem::take(&mut self.pending_jumps);
        for (offset, label) in pending {
            if let Some(&target) = self.block_offsets.get(&format!("{}:{}", func.name, label)) {
                self.patch_jump(offset, target);
            }
        }
    }
    
    /// Compile a basic block
    fn compile_block(&mut self, block: &HlirBasicBlock, func: &HlirFunction) {
        // Record block start offset
        let block_key = format!("{}:{}", func.name, block.name);
        let block_start = self.current_offset();
        self.block_offsets.insert(block_key, block_start);
        
        // Compile instructions
        for instr in &block.instructions {
            self.compile_instruction(instr);
        }
        
        // Compile terminator
        if let Some(term) = &block.terminator {
            self.compile_instruction(term);
        }
    }
    
    /// Compile a single instruction
    fn compile_instruction(&mut self, instr: &HlirInstr) {
        match instr {
            HlirInstr::BinOp { op, lhs, rhs, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let lhs_reg = self.get_value_reg(lhs);
                let rhs_reg = self.get_value_reg(rhs);
                
                let opcode = match op {
                    HlirBinOp::Add | HlirBinOp::FAdd => Opcode::Add,
                    HlirBinOp::Sub | HlirBinOp::FSub => Opcode::Subtract,
                    HlirBinOp::Mul | HlirBinOp::FMul => Opcode::Multiply,
                    HlirBinOp::SDiv | HlirBinOp::UDiv | HlirBinOp::FDiv => Opcode::Divide,
                    HlirBinOp::SRem | HlirBinOp::URem => Opcode::Modulo,
                    HlirBinOp::And => Opcode::And,
                    HlirBinOp::Or => Opcode::Or,
                    HlirBinOp::Xor => Opcode::BitXor,
                    HlirBinOp::Eq => Opcode::Equal,
                    HlirBinOp::Ne => Opcode::NotEqual,
                    HlirBinOp::Slt | HlirBinOp::Ult => Opcode::Less,
                    HlirBinOp::Sle | HlirBinOp::Ule => Opcode::LessEqual,
                    HlirBinOp::Sgt | HlirBinOp::Ugt => Opcode::Greater,
                    HlirBinOp::Sge | HlirBinOp::Uge => Opcode::GreaterEqual,
                    _ => Opcode::Nop,
                };
                
                self.emit_opcode(opcode);
                self.emit(dest_reg);
                self.emit(lhs_reg);
                self.emit(rhs_reg);
            }
            
            HlirInstr::UnaryOp { op, value, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let value_reg = self.get_value_reg(value);
                
                match op {
                    HlirUnaryOp::Neg => {
                        // Negate: 0 - value
                        let zero_reg = self.alloc_reg();
                        self.emit_opcode(Opcode::LoadInt);
                        self.emit(zero_reg);
                        self.emit_i64(0);
                        
                        self.emit_opcode(Opcode::Subtract);
                        self.emit(dest_reg);
                        self.emit(zero_reg);
                        self.emit(value_reg);
                    }
                    HlirUnaryOp::Not | HlirUnaryOp::LNot => {
                        self.emit_opcode(Opcode::Not);
                        self.emit(dest_reg);
                        self.emit(value_reg);
                    }
                }
            }
            
            HlirInstr::Alloca { ty, dest, name } => {
                // For alloca, we just allocate a register and track the variable
                let reg = self.alloc_reg();
                self.reg_map.insert(*dest, reg);
                self.var_map.insert(name.clone(), reg);
                // Note: Sparkler doesn't have explicit alloca, variables are just registers
            }
            
            HlirInstr::Load { ptr, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                
                // If ptr is a local variable, just move the value
                if let HlirValue::Temp(temp) = ptr {
                    if let Some(&var_reg) = self.var_map.get(&format!("_temp_{}", temp)) {
                        self.emit_opcode(Opcode::Move);
                        self.emit(dest_reg);
                        self.emit(var_reg);
                        return;
                    }
                }
                
                // Otherwise, just copy from source register
                let src_reg = self.get_value_reg(ptr);
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(src_reg);
            }
            
            HlirInstr::Store { value, ptr, ty } => {
                let value_reg = self.get_value_reg(value);
                
                // If storing to a local variable, track it
                if let HlirValue::Temp(temp) = ptr {
                    // This is storing to a temp - just move
                    if let Some(&dest_reg) = self.reg_map.get(temp) {
                        self.emit_opcode(Opcode::Move);
                        self.emit(dest_reg);
                        self.emit(value_reg);
                        return;
                    }
                }
                
                // Store to variable
                if let HlirValue::Temp(temp) = ptr {
                    if let Some(&var_reg) = self.var_map.get(&format!("_temp_{}", temp)) {
                        self.emit_opcode(Opcode::Move);
                        self.emit(var_reg);
                        self.emit(value_reg);
                        return;
                    }
                }
            }
            
            HlirInstr::Return { value, ty } => {
                if let Some(v) = value {
                    let value_reg = self.get_value_reg(v);
                    // Move to R0 (return register)
                    self.emit_opcode(Opcode::Move);
                    self.emit(0);
                    self.emit(value_reg);
                }
                self.emit_opcode(Opcode::Return);
                self.emit(0); // Rd (unused for Return)
            }
            
            HlirInstr::Br { target } => {
                self.emit_opcode(Opcode::Jump);
                // Placeholder for target offset (will be patched)
                let placeholder = self.current_offset();
                self.emit_u16(0);
                self.pending_jumps.push((placeholder, target.clone()));
            }
            
            HlirInstr::CondBr { cond, then_block, else_block } => {
                let cond_reg = self.get_value_reg(cond);
                
                // JumpIfFalse to else block
                self.emit_opcode(Opcode::JumpIfFalse);
                self.emit(cond_reg);
                let else_placeholder = self.current_offset();
                self.emit_u16(0);
                self.pending_jumps.push((else_placeholder, else_block.clone()));
                
                // Fall through to then block (implicit)
                // Add unconditional jump to end of then block
                self.emit_opcode(Opcode::Jump);
                let then_end_placeholder = self.current_offset();
                self.emit_u16(0);
                self.pending_jumps.push((then_end_placeholder, format!("{}_end", then_block)));
            }
            
            HlirInstr::Call { func, args, dest, return_ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                
                // Get function name
                if let HlirValue::Function(name) = func {
                    let func_idx = self.add_string(name.clone());
                    
                    // Emit arguments first (they need to be in consecutive registers)
                    let arg_start = self.next_sparkler_reg;
                    for arg in args {
                        let arg_reg = self.get_value_reg(arg);
                        // Move argument to consecutive register
                        let target_reg = self.alloc_reg();
                        self.emit_opcode(Opcode::Move);
                        self.emit(target_reg);
                        self.emit(arg_reg);
                    }
                    
                    self.emit_opcode(Opcode::Call);
                    self.emit(dest_reg);
                    self.emit(func_idx as u8);
                    self.emit(arg_start as u8);
                    self.emit(args.len() as u8);
                }
            }
            
            HlirInstr::Cmp { op, lhs, rhs, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let lhs_reg = self.get_value_reg(lhs);
                let rhs_reg = self.get_value_reg(rhs);
                
                let opcode = match op {
                    HlirBinOp::Eq => Opcode::Equal,
                    HlirBinOp::Ne => Opcode::NotEqual,
                    HlirBinOp::Slt => Opcode::Less,
                    HlirBinOp::Sle => Opcode::LessEqual,
                    HlirBinOp::Sgt => Opcode::Greater,
                    HlirBinOp::Sge => Opcode::GreaterEqual,
                    _ => Opcode::Equal,
                };
                
                self.emit_opcode(opcode);
                self.emit(dest_reg);
                self.emit(lhs_reg);
                self.emit(rhs_reg);
            }
            
            HlirInstr::Cast { value, from_ty, to_ty, dest, kind } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let value_reg = self.get_value_reg(value);
                
                // For now, just move the value (type conversion is a no-op in simple cases)
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(value_reg);
            }
            
            HlirInstr::Select { cond, then_val, else_val, dest, ty } => {
                // Select: dest = cond ? then_val : else_val
                let dest_reg = self.get_reg_for_temp(*dest);
                let cond_reg = self.get_value_reg(cond);
                let then_reg = self.get_value_reg(then_val);
                let else_reg = self.get_value_reg(else_val);
                
                // This is complex - need conditional move
                // For simplicity, use branches
                let else_label = format!("select_else_{}", dest);
                let end_label = format!("select_end_{}", dest);
                
                self.emit_opcode(Opcode::JumpIfFalse);
                self.emit(cond_reg);
                let else_placeholder = self.current_offset();
                self.emit_u16(0);
                
                // Then branch
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(then_reg);
                self.emit_opcode(Opcode::Jump);
                let end_placeholder = self.current_offset();
                self.emit_u16(0);
                
                // Patch else jump
                let else_offset = self.current_offset();
                self.patch_jump(else_placeholder, else_offset);
                
                // Else branch
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(else_reg);
                
                // Patch end jump
                let end_offset = self.current_offset();
                self.patch_jump(end_placeholder, end_offset);
            }
            
            _ => {
                // Unhandled instruction - emit NOP
                self.emit_opcode(Opcode::Nop);
            }
        }
    }
    
    /// Get or create a register for an HLIR value
    fn get_value_reg(&mut self, value: &HlirValue) -> u8 {
        match value {
            HlirValue::IntConst(n) => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadInt);
                self.emit(reg);
                self.emit_i64(*n);
                reg
            }
            HlirValue::FloatConst(n) => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadFloat);
                self.emit(reg);
                self.emit_f64(*n);
                reg
            }
            HlirValue::BoolConst(b) => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadBool);
                self.emit(reg);
                self.emit(if *b { 1 } else { 0 });
                reg
            }
            HlirValue::Param(n) => {
                // Parameters are in R1, R2, ...
                (*n + 1) as u8
            }
            HlirValue::Temp(n) => {
                self.get_reg_for_temp(*n)
            }
            HlirValue::Local(name) => {
                if let Some(&reg) = self.var_map.get(name) {
                    reg
                } else {
                    let reg = self.alloc_reg();
                    self.var_map.insert(name.clone(), reg);
                    reg
                }
            }
            _ => {
                // Default: allocate a register and load null
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadNull);
                self.emit(reg);
                reg
            }
        }
    }
    
    /// Get the compiled bytecode
    pub fn get_bytecode(&self) -> &[u8] {
        &self.bytecode
    }
    
    /// Get the string table
    pub fn get_strings(&self) -> &[String] {
        &self.strings
    }
}

impl Default for HlirToSparkler {
    fn default() -> Self {
        Self::new()
    }
}

/// Compile HLIR module to Sparkler bytecode
pub fn compile_hlir_to_sparkler(hlir: &HlirModule) -> CompiledBytecode {
    let mut compiler = HlirToSparkler::new();
    compiler.compile_module(hlir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlir::{HlirBuilder, HlirValue, HlirBinOp, HlirType};
    
    #[test]
    fn test_compile_simple_add() {
        // Create HLIR: fn add(a: i32, b: i32) -> i32 { return a + b; }
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
        let bytecode = compile_hlir_to_sparkler(&hlir);
        
        assert!(!bytecode.data.is_empty());
        assert!(bytecode.max_registers > 0);
    }
    
    #[test]
    fn test_compile_arithmetic() {
        // Create HLIR: fn compute(x: i32) -> i32 { return (x * 2) + 3; }
        let mut builder = HlirBuilder::new("arith");
        builder.begin_function("compute", vec![
            ("x".to_string(), HlirType::I32),
        ], HlirType::I32);
        builder.begin_block("entry");
        
        let x = HlirValue::Param(0);
        let two = HlirValue::IntConst(2);
        let three = HlirValue::IntConst(3);
        
        let mul = builder.bin_op(HlirBinOp::Mul, x, two, HlirType::I32);
        let add = builder.bin_op(HlirBinOp::Add, mul, three, HlirType::I32);
        
        builder.ret(Some(add), HlirType::I32);
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);
        
        assert!(!bytecode.data.is_empty());
    }
    
    #[test]
    fn test_compile_with_variables() {
        // Create HLIR with alloca and store/load
        let mut builder = HlirBuilder::new("vars");
        builder.begin_function("use_var", vec![], HlirType::I32);
        builder.begin_block("entry");
        
        let x_ptr = builder.alloca(HlirType::I32, "x");
        builder.store(HlirValue::IntConst(42), x_ptr.clone(), HlirType::I32);
        let loaded = builder.load(x_ptr, HlirType::I32);
        
        builder.ret(Some(loaded), HlirType::I32);
        builder.end_block();
        builder.end_function();
        
        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);
        
        assert!(!bytecode.data.is_empty());
    }
}

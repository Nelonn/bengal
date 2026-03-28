//! HLIR to Sparkler Bytecode Compiler
//!
//! This module compiles HLIR (High-Level IR) to Sparkler bytecode.

use crate::hlir::{HlirModule, HlirFunction, HlirBasicBlock, HlirInstr, HlirValue, HlirBinOp, HlirUnaryOp, HlirType, HlirClass};
use crate::types::{mangle, Type};
use sparkler::opcodes::Opcode;
use sparkler::vm::{Function, Class, VTable};

/// Convert HlirType to Type for mangling purposes
fn hlir_type_to_type(ty: &HlirType) -> Type {
    match ty {
        HlirType::Void => Type::Unknown,
        HlirType::Bool => Type::Bool,
        HlirType::I8 => Type::Int8,
        HlirType::I32 => Type::Int,
        HlirType::I64 => Type::Int64,
        HlirType::F32 => Type::Float32,
        HlirType::F64 => Type::Float,
        HlirType::String => Type::Str,
        HlirType::Array(inner) => Type::Array(Box::new(hlir_type_to_type(inner))),
        HlirType::Pointer(_) => Type::Unknown,
        HlirType::Function(_, _) => Type::Unknown,
        HlirType::Unknown => Type::Unknown,
    }
}

/// Compiled bytecode with metadata
#[derive(Clone)]
pub struct CompiledBytecode {
    pub data: Vec<u8>,
    pub strings: Vec<String>,
    pub max_registers: usize,
    pub functions: Vec<Function>,
    pub classes: Vec<Class>,
    pub vtables: Vec<VTable>,
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
    /// TRY_START catch_pc patches that need patching: (offset_in_bytecode, target_label)
    pending_try_starts: Vec<(usize, String)>,
    /// Registers allocated for constants in current instruction
    const_regs: Vec<u8>,
    /// Stack of reusable registers
    reusable_regs: Vec<u8>,
    /// Map from temp to its last use instruction index (for liveness tracking)
    temp_last_use: std::collections::HashMap<usize, usize>,
    /// Current instruction index being compiled
    current_instr_idx: usize,
    /// Reverse map: register -> set of temps using it (for proper release)
    reg_to_temps: std::collections::HashMap<u8, std::collections::HashSet<usize>>,
    /// Set of native function names that should use CallNative opcode
    native_functions: std::collections::HashSet<String>,
    /// Set of generic function names that should use * for mangling
    generic_functions: std::collections::HashSet<String>,
    /// Set of interface names for detecting interface method calls
    interface_names: std::collections::HashSet<String>,
    /// Map from class name to its vtable (method names in order)
    vtables: std::collections::HashMap<String, Vec<String>>,
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
            pending_try_starts: Vec::new(),
            const_regs: Vec::new(),
            reusable_regs: Vec::new(),
            temp_last_use: std::collections::HashMap::new(),
            current_instr_idx: 0,
            reg_to_temps: std::collections::HashMap::new(),
            native_functions: std::collections::HashSet::new(),
            generic_functions: std::collections::HashSet::new(),
            interface_names: std::collections::HashSet::new(),
            vtables: std::collections::HashMap::new(),
        }
    }

    /// Create a new HlirToSparkler with native and generic function information
    pub fn with_native_and_generic_functions(native_functions: Vec<String>, generic_functions: Vec<String>) -> Self {
        Self {
            bytecode: Vec::new(),
            strings: Vec::new(),
            reg_map: std::collections::HashMap::new(),
            next_sparkler_reg: 1,
            max_reg: 0,
            var_map: std::collections::HashMap::new(),
            block_offsets: std::collections::HashMap::new(),
            pending_jumps: Vec::new(),
            pending_try_starts: Vec::new(),
            const_regs: Vec::new(),
            reusable_regs: Vec::new(),
            temp_last_use: std::collections::HashMap::new(),
            current_instr_idx: 0,
            reg_to_temps: std::collections::HashMap::new(),
            native_functions: native_functions.into_iter().collect(),
            generic_functions: generic_functions.into_iter().collect(),
            interface_names: std::collections::HashSet::new(),
            vtables: std::collections::HashMap::new(),
        }
    }

    /// Allocate a new Sparkler register for a temp
    fn alloc_reg_for_temp(&mut self, temp: usize) -> u8 {
        if let Some(reg) = self.reusable_regs.pop() {
            self.reg_to_temps
                .entry(reg)
                .or_insert_with(std::collections::HashSet::new)
                .insert(temp);
            return reg;
        }

        let reg = self.next_sparkler_reg as u8;
        self.next_sparkler_reg += 1;
        if reg as u16 > self.max_reg {
            self.max_reg = reg as u16;
        }
        self.reg_to_temps
            .entry(reg)
            .or_insert_with(std::collections::HashSet::new)
            .insert(temp);
        reg
    }

    /// Allocate a new Sparkler register (for constants/temporaries, not liveness-tracked)
    fn alloc_reg(&mut self) -> u8 {
        // Don't reuse from reusable_regs to avoid conflicts with temps
        // that may still be using those registers
        let reg = self.next_sparkler_reg as u8;
        self.next_sparkler_reg += 1;
        if reg as u16 > self.max_reg {
            self.max_reg = reg as u16;
        }
        reg
    }

    /// Allocate a block of n consecutive registers (for call staging windows)
    fn alloc_consecutive_regs(&mut self, n: usize) -> u8 {
        if n == 0 {
            return 0;
        }
        if n == 1 {
            return self.alloc_reg();
        }

        // To ensure they are consecutive, we bypass reusable_regs for n > 1
        let reg = self.next_sparkler_reg as u8;
        self.next_sparkler_reg += n as u16;
        if self.next_sparkler_reg as u16 - 1 > self.max_reg {
            self.max_reg = self.next_sparkler_reg - 1;
        }
        reg
    }

    /// Get or create a Sparkler register for an HLIR temp
    fn get_reg_for_temp(&mut self, temp: usize) -> u8 {
        if let Some(&reg) = self.reg_map.get(&temp) {
            reg
        } else {
            let reg = self.alloc_reg_for_temp(temp);
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

    /// Patch a jump target at the given offset (uses absolute offset)
    fn patch_jump(&mut self, offset: usize, target: usize) {
        // The VM uses absolute PC offsets for jumps, not relative
        // Write in little-endian order: low byte first, then high byte
        self.bytecode[offset] = (target & 0xFF) as u8;
        self.bytecode[offset + 1] = ((target >> 8) & 0xFF) as u8;
    }

    /// Patch a TRY_START catch_pc (uses absolute offset)
    fn patch_try_catch_pc(&mut self, offset: usize, target: usize) {
        // TRY_START uses absolute PC offset, not relative
        // Write in little-endian order: low byte first, then high byte
        self.bytecode[offset] = (target & 0xFF) as u8;
        self.bytecode[offset + 1] = ((target >> 8) & 0xFF) as u8;
    }

    /// Analyze liveness: find the last use of each temp in a function
    fn analyze_liveness(&mut self, func: &HlirFunction) {
        self.temp_last_use.clear();
        let mut instr_idx = 0;

        for block in &func.blocks {
            for instr in &block.instructions {
                self.record_uses(instr, instr_idx);
                instr_idx += 1;
            }
            if let Some(term) = &block.terminator {
                self.record_uses(term, instr_idx);
                instr_idx += 1;
            }
        }
    }

    /// Record uses of temps in an instruction (for liveness analysis)
    fn record_uses(&mut self, instr: &HlirInstr, instr_idx: usize) {
        let used_temps: Vec<usize> = match instr {
            HlirInstr::BinOp { lhs, rhs, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = lhs { temps.push(*t); }
                if let HlirValue::Temp(t) = rhs { temps.push(*t); }
                temps
            }
            HlirInstr::UnaryOp { value, .. } => {
                if let HlirValue::Temp(t) = value { vec![*t] } else { vec![] }
            }
            HlirInstr::Load { ptr, .. } => {
                if let HlirValue::Temp(t) = ptr { vec![*t] } else { vec![] }
            }
            HlirInstr::Store { value, ptr, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = value { temps.push(*t); }
                if let HlirValue::Temp(t) = ptr { temps.push(*t); }
                temps
            }
            HlirInstr::Call { func, args, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = func { temps.push(*t); }
                for arg in args {
                    if let HlirValue::Temp(t) = arg { temps.push(*t); }
                }
                temps
            }
            HlirInstr::Cmp { lhs, rhs, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = lhs { temps.push(*t); }
                if let HlirValue::Temp(t) = rhs { temps.push(*t); }
                temps
            }
            HlirInstr::Cast { value, .. } => {
                if let HlirValue::Temp(t) = value { vec![*t] } else { vec![] }
            }
            HlirInstr::Select { cond, then_val, else_val, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = cond { temps.push(*t); }
                if let HlirValue::Temp(t) = then_val { temps.push(*t); }
                if let HlirValue::Temp(t) = else_val { temps.push(*t); }
                temps
            }
            HlirInstr::CondBr { cond, .. } => {
                if let HlirValue::Temp(t) = cond { vec![*t] } else { vec![] }
            }
            HlirInstr::Phi { sources, .. } => {
                let mut temps = Vec::new();
                for (val, _) in sources {
                    if let HlirValue::Temp(t) = val { temps.push(*t); }
                }
                temps
            }
            HlirInstr::GetElementPtr { base, indices, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = base { temps.push(*t); }
                for idx in indices {
                    if let HlirValue::Temp(t) = idx { temps.push(*t); }
                }
                temps
            }
            HlirInstr::StringConcat { values, .. } => {
                let mut temps = Vec::new();
                for value in values {
                    if let HlirValue::Temp(t) = value { temps.push(*t); }
                }
                temps
            }
            HlirInstr::SetProperty { object, value, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = object { temps.push(*t); }
                if let HlirValue::Temp(t) = value { temps.push(*t); }
                temps
            }
            HlirInstr::GetProperty { object, dest, .. } => {
                let mut temps = Vec::new();
                if let HlirValue::Temp(t) = object { temps.push(*t); }
                temps.push(*dest);
                temps
            }
            _ => vec![],
        };

        for temp in used_temps {
            self.temp_last_use.insert(temp, instr_idx);
        }
    }

    /// Release registers for temps that are no longer needed after current instruction
    fn release_dead_temps(&mut self) {
        let dead_temps: Vec<usize> = self.reg_map
            .keys()
            .copied()
            .filter(|&temp| {
                self.temp_last_use
                    .get(&temp)
                    .map_or(true, |&last_use| self.current_instr_idx > last_use)
            })
            .collect();

        let mut reg_release_check: std::collections::HashSet<u8> =
            std::collections::HashSet::new();

        for temp in dead_temps {
            if let Some(reg) = self.reg_map.remove(&temp) {
                if let Some(temps) = self.reg_to_temps.get_mut(&reg) {
                    temps.remove(&temp);
                    if temps.is_empty() {
                        // Don't free registers that hold local variables
                        let is_local_reg = self.var_map.values().any(|&r| r == reg);
                        if !is_local_reg {
                            reg_release_check.insert(reg);
                        }
                    }
                }
            }
        }

        for reg in reg_release_check {
            self.reusable_regs.push(reg);
            self.reg_to_temps.remove(&reg);
        }
    }

    /// Compile an HLIR module to Sparkler bytecode
    pub fn compile_module(&mut self, hlir: &HlirModule) -> CompiledBytecode {
        // Collect interface names for detecting interface method calls
        for class in &hlir.classes {
            if class.is_interface {
                self.interface_names.insert(class.name.clone());
            }
            // Store vtable for all classes (not just interfaces) that implement interfaces
            if !class.vtable.is_empty() {
                self.vtables.insert(class.name.clone(), class.vtable.clone());
            }
        }

        // First pass: collect function info and block offsets
        for func in &hlir.functions {
            let mut offset = self.current_offset();
            for block in &func.blocks {
                self.block_offsets
                    .insert(format!("{}:{}", func.name, block.name), offset);
                offset += block.instructions.len() * 4 + 10;
            }
        }

        // Track function bytecode ranges
        let mut functions = Vec::new();

        // Second pass: compile functions and track their bytecode
        for func in &hlir.functions {
            let func_start = self.current_offset();
            self.compile_function(func);
            let func_end = self.current_offset();

            // Extract function bytecode
            let func_bytecode = self.bytecode[func_start..func_end].to_vec();

            // Mangle function name if it's generic
            let func_name = if self.generic_functions.iter().any(|gf| {
                let gf_base = gf.split('(').next().unwrap_or(gf);
                func.name == gf_base || func.name.ends_with(&format!(".{}", gf_base))
            }) {
                // For generic functions, add (*) suffix
                format!("{}(*)", func.name)
            } else {
                func.name.clone()
            };

            functions.push(Function {
                name: func_name,
                bytecode: func_bytecode,
                param_count: func.params.len() as u8,
                register_count: self.max_reg as u8 + 1,
                source_file: None,
            });
        }

        // Generate root section that calls the module wrapper
        // Root ALWAYS calls "<module_name>._main" - the module entry point
        // For main module (no module prefix), the function is just "_main"
        let main_function_name = {
            let expected_wrapper_name = format!("{}._main", hlir.name);
            // First try with module prefix (e.g., "std.io._main")
            hlir.functions.iter()
                .find(|f| f.name == expected_wrapper_name)
                .map(|f| f.name.clone())
                // Fall back to just "_main" for main module (when module_prefix is empty)
                .or_else(|| hlir.functions.iter()
                    .find(|f| f.name == "_main")
                    .map(|f| f.name.clone()))
        };

        let root_bytecode = if let Some(main_name) = main_function_name {
            // Generate CALL instruction to call main function
            let mut root = Vec::new();
            let func_idx = self.add_string(main_name.clone());
            
            // CALL instruction format: [CALL, Rd, func_idx, arg_start, arg_count]
            // Rd = R0 (return value), arg_start = R0, arg_count = 0 (no args)
            root.push(Opcode::Call as u8);
            root.push(0);        // Rd = R0 (return value)
            root.push(func_idx as u8);  // function name string index
            root.push(0);        // arg_start = R0
            root.push(0);        // arg_count = 0 (no arguments)
            
            // RETURN R0
            root.push(Opcode::Return as u8);
            root.push(0);        // Return R0

            root
        } else {
            // No main function - just return Null
            vec![Opcode::Return as u8, 0]
        };

        // Generate Class metadata from HLIR class info
        let classes: Vec<Class> = hlir.classes.iter().map(|hlir_class| {
            use std::collections::{HashMap, HashSet};

            // Create default field values (Null for all fields)
            let mut fields: HashMap<String, sparkler::vm::Value> = HashMap::new();
            for field_name in &hlir_class.fields {
                fields.insert(field_name.clone(), sparkler::vm::Value::Null);
            }

            let private_fields: HashSet<String> = hlir_class.private_fields.iter().cloned().collect();

            // Populate methods from the functions list
            // Methods are stored with mangled names like "Circle_print()"
            let mut methods: HashMap<String, sparkler::vm::Method> = HashMap::new();
            for method_name in &hlir_class.methods {
                // Find the corresponding function
                if let Some(func) = functions.iter().find(|f| &f.name == method_name) {
                    methods.insert(method_name.clone(), sparkler::vm::Method {
                        name: method_name.clone(),
                        bytecode: func.bytecode.clone(),
                        register_count: func.register_count,
                    });
                }
            }
            
            let native_methods: HashMap<String, sparkler::vm::NativeFn> = HashMap::new();

            Class {
                name: hlir_class.name.clone(),
                fields,
                private_fields,
                methods,
                native_methods,
                native_create: None,
                native_destroy: None,
                is_native: hlir_class.is_native,
                parent_interfaces: hlir_class.parent_interfaces.clone(),
                vtable: hlir_class.vtable.clone(),
                is_interface: hlir_class.is_interface,
            }
        }).collect();

        CompiledBytecode {
            data: root_bytecode,
            strings: self.strings.clone(),
            max_registers: self.max_reg as usize + 1,
            functions,
            classes,
            vtables: Vec::new(),
        }
    }

    /// Compile a single function
    fn compile_function(&mut self, func: &HlirFunction) {
        // Reset state for new function
        self.reg_map.clear();
        self.var_map.clear();
        self.temp_last_use.clear();
        self.reusable_regs.clear();
        self.reg_to_temps.clear();
        self.next_sparkler_reg = 1;
        self.max_reg = 0;
        self.current_instr_idx = 0;

        // Analyze liveness first
        self.analyze_liveness(func);

        // Allocate registers for parameters (R1, R2, ...)
        for (i, (name, _)) in func.params.iter().enumerate() {
            let reg = (i + 1) as u8;
            self.var_map.insert(name.clone(), reg);
            if reg as u16 > self.max_reg {
                self.max_reg = reg as u16;
            }
        }

        // Record the start offset of this function's bytecode for relative jump calculations
        let func_start_offset = self.current_offset();

        // Compile blocks
        for block in &func.blocks {
            self.compile_block(block, func);
        }

        // Patch pending jumps - convert absolute offsets to relative offsets
        let pending = std::mem::take(&mut self.pending_jumps);
        for (offset, label) in pending {
            let block_key = format!("{}:{}", func.name, label);
            if let Some(&target) = self.block_offsets.get(&block_key) {
                // Convert absolute offset to relative offset (relative to function start)
                let relative_target = target - func_start_offset;
                self.patch_jump(offset, relative_target);
            }
        }

        // Patch pending TRY_START catch_pc (uses absolute offset)
        let pending_try_starts = std::mem::take(&mut self.pending_try_starts);
        for (offset, label) in pending_try_starts {
            if let Some(&target) =
                self.block_offsets.get(&format!("{}:{}", func.name, label))
            {
                self.patch_try_catch_pc(offset, target);
            }
        }
    }

    /// Compile a basic block
    fn compile_block(&mut self, block: &HlirBasicBlock, func: &HlirFunction) {
        let block_key = format!("{}:{}", func.name, block.name);
        let block_start = self.current_offset();
        self.block_offsets.insert(block_key, block_start);

        for instr in &block.instructions {
            self.release_dead_temps();
            self.compile_instruction(instr);
            self.current_instr_idx += 1;
        }

        if let Some(term) = &block.terminator {
            self.release_dead_temps();
            self.compile_instruction(term);
            self.current_instr_idx += 1;
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
                    HlirBinOp::Add => {
                        // String concatenation uses Concat opcode
                        if *ty == HlirType::String {
                            Opcode::Concat
                        } else {
                            Opcode::Add
                        }
                    }
                    HlirBinOp::FAdd => Opcode::Add,
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
                if opcode == Opcode::Concat {
                    // Concat: Rd, rs_start, count
                    // For binary concat, count is always 2
                    self.emit(dest_reg);
                    self.emit(lhs_reg);
                    self.emit(2u8);
                } else {
                    self.emit(dest_reg);
                    self.emit(lhs_reg);
                    self.emit(rhs_reg);
                }
            }

            HlirInstr::UnaryOp { op, value, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let value_reg = self.get_value_reg(value);

                match op {
                    HlirUnaryOp::Neg => {
                        // FIX: push zero_reg to const_regs so it is freed after this instruction
                        let zero_reg = self.alloc_reg();
                        self.const_regs.push(zero_reg);

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
                let reg = self.alloc_reg();
                self.reg_map.insert(*dest, reg);
                self.var_map.insert(name.clone(), reg);
            }

            HlirInstr::Load { ptr, dest, ty } => {
                // Copy propagation: reuse the source register directly when possible
                if let HlirValue::Temp(temp) = ptr {
                    if let Some(&src_reg) = self.reg_map.get(temp) {
                        self.reg_map.insert(*dest, src_reg);
                        self.reg_to_temps
                            .entry(src_reg)
                            .or_insert_with(std::collections::HashSet::new)
                            .insert(*dest);
                        return;
                    }
                }

                let dest_reg = self.alloc_reg_for_temp(*dest);
                self.reg_map.insert(*dest, dest_reg);

                let src_reg = self.get_value_reg(ptr);
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(src_reg);
            }

            HlirInstr::Store { value, ptr, ty } => {
                let value_reg = self.get_value_reg(value);

                if let HlirValue::Temp(temp) = ptr {
                    if let Some(&dest_reg) = self.reg_map.get(temp) {
                        self.emit_opcode(Opcode::Move);
                        self.emit(dest_reg);
                        self.emit(value_reg);
                        return;
                    }
                }

                if let HlirValue::Temp(temp) = ptr {
                    if let Some(&var_reg) =
                        self.var_map.get(&format!("_temp_{}", temp))
                    {
                        self.emit_opcode(Opcode::Move);
                        self.emit(var_reg);
                        self.emit(value_reg);
                    }
                }
            }

            HlirInstr::Return { value, ty } => {
                if let Some(v) = value {
                    let value_reg = self.get_value_reg(v);
                    self.emit_opcode(Opcode::Move);
                    self.emit(0);
                    self.emit(value_reg);
                }
                self.emit_opcode(Opcode::Return);
                self.emit(0);
            }

            HlirInstr::Br { target } => {
                self.emit_opcode(Opcode::Jump);
                let placeholder = self.current_offset();
                self.emit_u16(0);
                self.pending_jumps.push((placeholder, target.clone()));
            }

            HlirInstr::CondBr { cond, then_block, else_block } => {
                let cond_reg = self.get_value_reg(cond);

                self.emit_opcode(Opcode::JumpIfFalse);
                self.emit(cond_reg);
                let else_placeholder = self.current_offset();
                self.emit_u16(0);
                self.pending_jumps.push((else_placeholder, else_block.clone()));
            }

            HlirInstr::TryStart { catch_block, catch_reg } => {
                // Allocate a register for the exception
                let exc_reg = self.get_reg_for_temp(*catch_reg);

                // Emit TryStart opcode: [TryStart, catch_pc (2 bytes), catch_reg]
                self.emit_opcode(Opcode::TryStart);
                let placeholder = self.current_offset();
                self.emit_u16(0);  // Placeholder for catch_pc
                self.emit(exc_reg);

                // Store the pending try_start to patch later (uses absolute offset)
                self.pending_try_starts.push((placeholder, catch_block.clone()));
            }

            HlirInstr::TryEnd => {
                self.emit_opcode(Opcode::TryEnd);
            }

            HlirInstr::Throw { value } => {
                let value_reg = self.get_value_reg(value);
                self.emit_opcode(Opcode::Throw);
                self.emit(value_reg);
            }

            HlirInstr::SetProperty { object, field_name, value } => {
                // Emit SetProperty: Robj, name_idx, Rs
                let object_reg = self.get_value_reg(object);
                let value_reg = self.get_value_reg(value);
                let field_idx = self.add_string(field_name.clone());

                self.emit_opcode(Opcode::SetProperty);
                self.emit(object_reg);
                self.emit(field_idx as u8);
                self.emit(value_reg);
            }

            HlirInstr::GetProperty { object, field_name, dest } => {
                // Emit GetProperty: Rd, Robj, name_idx
                let dest_reg = self.get_reg_for_temp(*dest);
                let object_reg = self.get_value_reg(object);
                let field_idx = self.add_string(field_name.clone());

                self.emit_opcode(Opcode::GetProperty);
                self.emit(dest_reg);
                self.emit(object_reg);
                self.emit(field_idx as u8);
            }

            HlirInstr::Call { func, args, dest, return_ty, arg_types } => {
                // Only allocate dest register if the return value is used
                let dest_reg = dest.map(|d| self.get_reg_for_temp(d)).unwrap_or(0);

                if let HlirValue::Function(name) = func {
                    // Check if this is an interface method call
                    // Pattern: Interface.method(args) where Interface is an interface name
                    // The method name format from ast_to_hlir_full.rs is "Class.method(args)"
                    let is_interface_call = if let Some(dot_pos) = name.find('.') {
                        let potential_interface = &name[..dot_pos];
                        self.interface_names.contains(potential_interface)
                    } else {
                        false
                    };

                    if is_interface_call && !args.is_empty() {
                        // This is an interface method call - use InvokeInterface
                        // Extract interface name and method name
                        let dot_pos = name.find('.').unwrap();
                        let interface_name = &name[..dot_pos];
                        let method_with_suffix = &name[dot_pos + 1..];
                        // Extract base method name (without parameters)
                        let method_name = if let Some(paren_pos) = method_with_suffix.find('(') {
                            method_with_suffix[..paren_pos].to_string()
                        } else {
                            method_with_suffix.to_string()
                        };

                        // Find the vtable index for this method in the interface's vtable
                        let method_vtable_idx = if let Some(vtable) = self.vtables.get(interface_name) {
                            vtable.iter().position(|m| m == &method_name).unwrap_or(0) as u8
                        } else {
                            0u8
                        };

                        // All arguments including self (object)
                        // args[0] is the object (self), args[1..] are the method arguments
                        // Step 1: resolve each argument to a register
                        let src_regs: Vec<u8> = args
                            .iter()
                            .map(|arg| self.get_value_reg(arg))
                            .collect();

                        // Step 2: Check if source registers are already consecutive
                        let (arg_start, staging_regs, needs_moves) = if args.is_empty() {
                            (0, vec![], false)
                        } else if args.len() == 1 {
                            (src_regs[0], vec![src_regs[0]], false)
                        } else if src_regs.windows(2).all(|w| w[1] == w[0] + 1) {
                            (src_regs[0], src_regs.clone(), false)
                        } else {
                            let arg_start = self.alloc_consecutive_regs(args.len());
                            let staging_regs: Vec<u8> = (0..args.len())
                                .map(|i| arg_start + i as u8)
                                .collect();
                            (arg_start, staging_regs, true)
                        };

                        // Emit MOVEs only if source registers weren't consecutive
                        if needs_moves {
                            for (i, &src_reg) in src_regs.iter().enumerate() {
                                let staging_reg = staging_regs[i];
                                self.emit_opcode(Opcode::Move);
                                self.emit(staging_reg);
                                self.emit(src_reg);
                            }
                        }

                        // InvokeInterface: Rd, method_idx, arg_start, arg_count
                        self.emit_opcode(Opcode::InvokeInterface);
                        self.emit(dest_reg);
                        self.emit(method_vtable_idx);
                        self.emit(arg_start);
                        self.emit(args.len() as u8);

                        // Free staging registers (only if we allocated them)
                        if needs_moves {
                            for reg in staging_regs {
                                self.reusable_regs.push(reg);
                            }
                        }
                    } else {
                        // Regular function/method call - use existing logic
                        // Check if this is a generic function - if so, use * for all argument types
                        // Extract base name (without mangled suffix)
                        let base_name = name.split('(').next().unwrap_or(name);

                        // Check if the base name matches any generic function
                        let is_generic = self.generic_functions.iter().any(|gf| {
                            // Extract base name from generic function (without mangled suffix)
                            let gf_base = gf.split('(').next().unwrap_or(gf);
                            // Check for exact match or qualified match
                            base_name == gf_base || base_name.ends_with(&format!(".{}", gf_base))
                        });

                        // Step 3: emit the call - use CallNative for native functions
                        // Check if this is a native function by name (try exact match or prefix match)
                        let is_native = self.native_functions.contains(name.as_str())
                            || self.native_functions.iter().any(|nf| {
                                // Check if name starts with the native function name followed by '('
                                name.starts_with(nf.as_str()) && name[nf.len()..].starts_with('(')
                            });

                        // Mangle the function name with argument types for proper resolution
                        let arg_type_list: Vec<Type> = if is_generic {
                            // For generic functions, use * for all arguments
                            vec![Type::Unknown; arg_types.len()]
                        } else if is_native {
                            // For native functions, try to use concrete types
                            // If types are Unknown, fall back to * for compatibility
                            let types: Vec<Type> = arg_types.iter().map(hlir_type_to_type).collect();
                            if types.iter().all(|t| matches!(t, Type::Unknown)) {
                                // All types are Unknown, use * for each argument
                                vec![Type::Unknown; arg_types.len()]
                            } else {
                                types
                            }
                        } else {
                            // For Bengal functions, use concrete types if available
                            arg_types.iter().map(hlir_type_to_type).collect()
                        };
                        let mangled_name = mangle(None, None, name, &arg_type_list);
                        let func_idx = self.add_string(mangled_name);

                        // --- Optimized call argument handling ---
                        // Step 1: resolve each argument to a register.
                        let src_regs: Vec<u8> = args
                            .iter()
                            .map(|arg| self.get_value_reg(arg))
                            .collect();

                        // Step 2: Check if source registers are already consecutive.
                        // If yes, reuse them directly without MOVEs.
                        let (arg_start, staging_regs, needs_moves) = if args.is_empty() {
                            // No arguments: use R0 as arg_start
                            (0, vec![], false)
                        } else if args.len() == 1 {
                            // Single argument: use its register directly
                            (src_regs[0], vec![src_regs[0]], false)
                        } else if src_regs.windows(2).all(|w| w[1] == w[0] + 1) {
                            // All registers are consecutive: use them directly
                            (src_regs[0], src_regs.clone(), false)
                        } else {
                            // Not consecutive: allocate staging window and emit MOVEs
                            let arg_start = self.alloc_consecutive_regs(args.len());
                            let staging_regs: Vec<u8> = (0..args.len())
                                .map(|i| arg_start + i as u8)
                                .collect();
                            (arg_start, staging_regs, true)
                        };

                        // Emit MOVEs only if source registers weren't consecutive
                        if needs_moves {
                            for (i, &src_reg) in src_regs.iter().enumerate() {
                                let staging_reg = staging_regs[i];
                                self.emit_opcode(Opcode::Move);
                                self.emit(staging_reg);
                                self.emit(src_reg);
                            }
                        }

                        // Step 3: emit the call
                        if is_native {
                            self.emit_opcode(Opcode::CallNative);
                            self.emit(dest_reg);
                            self.emit(func_idx as u8);
                            self.emit(arg_start);
                            self.emit(args.len() as u8);
                        } else {
                            self.emit_opcode(Opcode::Call);
                            self.emit(dest_reg);
                            self.emit(func_idx as u8);
                            self.emit(arg_start);
                            self.emit(args.len() as u8);
                        }

                        // Step 4: return staging registers to freelist (only if we allocated them)
                        if needs_moves {
                            for reg in staging_regs {
                                self.reusable_regs.push(reg);
                            }
                        }
                    }
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

                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(value_reg);
            }

            HlirInstr::Select { cond, then_val, else_val, dest, ty } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let cond_reg = self.get_value_reg(cond);
                let then_reg = self.get_value_reg(then_val);
                let else_reg = self.get_value_reg(else_val);

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

            HlirInstr::Phi { sources, dest, ty: _ } => {
                if let Some((first_val, _)) = sources.first() {
                    let dest_reg = self.get_reg_for_temp(*dest);
                    let src_reg = self.get_value_reg(first_val);
                    self.emit_opcode(Opcode::Move);
                    self.emit(dest_reg);
                    self.emit(src_reg);
                }
            }

            HlirInstr::GetElementPtr { base, indices, dest, ty: _ } => {
                let dest_reg = self.get_reg_for_temp(*dest);
                let base_reg = self.get_value_reg(base);
                self.emit_opcode(Opcode::Move);
                self.emit(dest_reg);
                self.emit(base_reg);
            }

            HlirInstr::StringConcat { values, dest } => {
                // Optimized string concatenation: single CONCAT opcode with all parts
                // CONCAT Rd, rs_start, count
                // Note: CONCAT requires values in CONTIGUOUS registers starting from rs_start
                let dest_reg = self.get_reg_for_temp(*dest);
                
                // First, evaluate all values and collect their registers
                let mut value_regs: Vec<u8> = Vec::new();
                for value in values {
                    let reg = self.get_value_reg(value);
                    value_regs.push(reg);
                }

                // Allocate contiguous registers for the CONCAT operation
                let start_reg = self.alloc_consecutive_regs(values.len());

                // Copy values to contiguous registers
                for (i, &value_reg) in value_regs.iter().enumerate() {
                    let target_reg = start_reg + i as u8;
                    if value_reg != target_reg {
                        self.emit_opcode(Opcode::Move);
                        self.emit(target_reg);
                        self.emit(value_reg);
                        // Mark source register as constant so it gets released
                        // (only release if we moved the value, otherwise the register
                        // is still in use by CONCAT and cannot be reused)
                        self.const_regs.push(value_reg);
                    }
                }
                
                // Emit CONCAT: Rd, rs_start, count
                self.emit_opcode(Opcode::Concat);
                self.emit(dest_reg);
                self.emit(start_reg);  // rs_start - first contiguous register
                self.emit(values.len() as u8);  // count
            }
        }

        // Release all constant/scratch registers allocated during this instruction
        for reg in self.const_regs.drain(..) {
            self.reusable_regs.push(reg);
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
                self.const_regs.push(reg);
                reg
            }
            HlirValue::FloatConst(n) => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadFloat);
                self.emit(reg);
                self.emit_f64(*n);
                self.const_regs.push(reg);
                reg
            }
            HlirValue::BoolConst(b) => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadBool);
                self.emit(reg);
                self.emit(if *b { 1 } else { 0 });
                self.const_regs.push(reg);
                reg
            }
            HlirValue::StringConst(s) => {
                let reg = self.alloc_reg();
                let str_idx = self.add_string(s.clone());
                self.emit_opcode(Opcode::LoadConst);
                self.emit(reg);
                self.emit_u16(str_idx as u16);
                self.const_regs.push(reg);
                reg
            }
            HlirValue::Param(n) => {
                // Parameters are fixed in R1, R2, … — never freed
                (*n + 1) as u8
            }
            HlirValue::Function(name) => {
                let reg = self.alloc_reg();
                let str_idx = self.add_string(name.clone());
                self.emit_opcode(Opcode::LoadConst);
                self.emit(reg);
                self.emit_u16(str_idx as u16);
                self.const_regs.push(reg);
                reg
            }
            HlirValue::Temp(n) => self.get_reg_for_temp(*n),
            HlirValue::Local(name) => {
                if let Some(&reg) = self.var_map.get(name) {
                    reg
                } else {
                    let reg = self.alloc_reg();
                    self.var_map.insert(name.clone(), reg);
                    reg
                }
            }
            HlirValue::Null => {
                let reg = self.alloc_reg();
                self.emit_opcode(Opcode::LoadNull);
                self.emit(reg);
                self.const_regs.push(reg);
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

/// Compile HLIR module to Sparkler bytecode with native function information
pub fn compile_hlir_to_sparkler_with_natives(hlir: &HlirModule, native_functions: Vec<String>, generic_functions: Vec<String>) -> CompiledBytecode {
    let mut compiler = HlirToSparkler::with_native_and_generic_functions(native_functions, generic_functions);
    compiler.compile_module(hlir)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlir::{HlirBuilder, HlirValue, HlirBinOp, HlirType};

    #[test]
    fn test_compile_simple_add() {
        let mut builder = HlirBuilder::new("test");
        builder.begin_function(
            "add",
            vec![
                ("a".to_string(), HlirType::I32),
                ("b".to_string(), HlirType::I32),
            ],
            HlirType::I32,
        );
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
        let mut builder = HlirBuilder::new("arith");
        builder.begin_function(
            "compute",
            vec![("x".to_string(), HlirType::I32)],
            HlirType::I32,
        );
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

    #[test]
    fn test_register_reuse() {
        let mut builder = HlirBuilder::new("reuse");
        builder.begin_function("test", vec![], HlirType::I32);
        builder.begin_block("entry");

        let a = builder.bin_op(
            HlirBinOp::Add, HlirValue::IntConst(1), HlirValue::IntConst(2), HlirType::I32,
        );
        let b = builder.bin_op(
            HlirBinOp::Add, HlirValue::IntConst(3), HlirValue::IntConst(4), HlirType::I32,
        );
        let c = builder.bin_op(
            HlirBinOp::Add, HlirValue::IntConst(5), HlirValue::IntConst(6), HlirType::I32,
        );

        builder.ret(Some(c), HlirType::I32);
        builder.end_block();
        builder.end_function();

        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);

        assert!(
            bytecode.max_registers < 10,
            "Register reuse not working: max_registers = {}",
            bytecode.max_registers
        );
    }

    #[test]
    fn test_register_reuse_sequential_calls() {
        let mut builder = HlirBuilder::new("calls");
        builder.begin_function("test", vec![], HlirType::Void);
        builder.begin_block("entry");

        let print_fn = HlirValue::Function("print".to_string());

        builder.call(print_fn.clone(), vec![HlirValue::IntConst(1)], HlirType::Void);
        builder.call(print_fn.clone(), vec![HlirValue::IntConst(2)], HlirType::Void);
        builder.call(print_fn.clone(), vec![HlirValue::IntConst(3)], HlirType::Void);

        builder.ret(None, HlirType::Void);
        builder.end_block();
        builder.end_function();

        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);

        assert!(
            bytecode.max_registers < 20,
            "Register reuse not working for calls: max_registers = {}",
            bytecode.max_registers
        );
    }

    /// Regression test: 50 sequential calls with zero variables must not blow
    /// up the register count. Before the fix this produced 100+ registers.
    #[test]
    fn test_register_reuse_many_calls() {
        let mut builder = HlirBuilder::new("many_calls");
        builder.begin_function("test", vec![], HlirType::Void);
        builder.begin_block("entry");

        let print_fn = HlirValue::Function("print".to_string());

        for i in 0..50_i64 {
            builder.call(
                print_fn.clone(),
                vec![HlirValue::IntConst(i)],
                HlirType::Void,
            );
        }

        builder.ret(None, HlirType::Void);
        builder.end_block();
        builder.end_function();

        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);

        assert!(
            bytecode.max_registers < 20,
            "Register leak on many calls: max_registers = {}",
            bytecode.max_registers
        );
    }

    /// Regression test: 50 calls with two arguments each.
    #[test]
    fn test_register_reuse_many_calls_multi_arg() {
        let mut builder = HlirBuilder::new("multi_arg_calls");
        builder.begin_function("test", vec![], HlirType::Void);
        builder.begin_block("entry");

        let println_fn = HlirValue::Function("println".to_string());

        for i in 0..50_i64 {
            builder.call(
                println_fn.clone(),
                vec![HlirValue::IntConst(i), HlirValue::IntConst(i * 2)],
                HlirType::Void,
            );
        }

        builder.ret(None, HlirType::Void);
        builder.end_block();
        builder.end_function();

        let hlir = builder.build();
        let bytecode = compile_hlir_to_sparkler(&hlir);

        assert!(
            bytecode.max_registers < 20,
            "Register leak on many multi-arg calls: max_registers = {}",
            bytecode.max_registers
        );
    }
}

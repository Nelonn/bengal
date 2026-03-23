use crate::parser::{Stmt, Expr, Literal, Parser, ClassDef, FunctionDef, BinaryOp, UnaryOp, InterpPart, CastType};
use crate::lexer::Lexer;
use crate::resolver::ModuleResolver;
use crate::types::{TypeContext, Type};
use sparkler::vm::{Class, Value, Function, Method};
use sparkler::opcodes::Opcode;

pub type Bytecode = sparkler::executor::Bytecode;

/// Add a string to the string table, reusing existing index if duplicate
fn add_string(strings: &mut Vec<String>, s: String) -> usize {
    strings.iter().position(|existing| *existing == s)
        .unwrap_or_else(|| {
            let idx = strings.len();
            strings.push(s);
            idx
        })
}

/// Extract base class name from generic type syntax (e.g., "Array<int>" -> "Array")
fn extract_base_class_name(name: &str) -> &str {
    if let Some(angle_pos) = name.find('<') {
        &name[..angle_pos]
    } else {
        name
    }
}

/// Adjust string indices in bytecode by adding an offset
/// This is needed when merging local string tables into a global one
fn adjust_string_indices(bytecode: &mut Vec<u8>, offset: usize) {
    let mut i = 0;
    while i < bytecode.len() {
        let opcode = bytecode[i];
        match opcode {
            // Nop, TryEnd, Breakpoint, Halt: 1 byte
            0x00 | 0x81 | 0x90 | 0xFF => {
                i += 1;
            }
            // LoadConst: Rd, string_idx (3 bytes)
            0x10 => {
                if i + 2 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 1;
                } else { i += 1; }
            }
            // LoadInt, LoadFloat: Rd, 8 bytes (10 bytes)
            0x11 | 0x12 => {
                i += 10;
            }
            // LoadBool: Rd, 1 byte (3 bytes)
            0x13 => {
                i += 3;
            }
            // LoadNull, Return, Throw: Rd/Rs (2 bytes)
            0x14 | 0x43 | 0x82 => {
                i += 2;
            }
            // Move, Not: Rd, Rs (3 bytes)
            0x20 | 0x64 => {
                i += 3;
            }
            // LoadLocal: Rd, name_idx (3 bytes)
            0x21 => {
                if i + 2 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 1;
                } else { i += 1; }
            }
            // StoreLocal: name_idx, Rs (3 bytes)
            0x22 => {
                if i + 2 < bytecode.len() {
                    i += 1; // skip opcode
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 2;
                } else { i += 1; }
            }
            // GetProperty: Rd, Robj, name_idx (4 bytes)
            0x30 => {
                if i + 3 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    i += 1; // skip Robj
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 1;
                } else { i += 1; }
            }
            // SetProperty: Robj, name_idx, Rs (4 bytes)
            0x31 => {
                if i + 3 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Robj
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 2;
                } else { i += 1; }
            }
            // Call: Rd, func_idx, arg_start, arg_count (5 bytes)
            0x40 => {
                if i + 4 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 3;
                } else { i += 1; }
            }
            // CallNative: Rd, name_idx, arg_start, arg_count (5 bytes)
            0x41 | 0x45 => {
                if i + 4 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 3;
                } else { i += 1; }
            }
            // Invoke: Rd, method_idx, arg_start, arg_count (5 bytes)
            0x42 | 0x46 => {
                if i + 4 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 3;
                } else { i += 1; }
            }
            // InvokeInterface: Rd, method_idx, arg_start, arg_count (5 bytes)
            0x49 | 0x4A => {
                if i + 4 < bytecode.len() {
                    i += 1; // skip opcode
                    i += 1; // skip Rd
                    let idx = bytecode[i] as usize;
                    bytecode[i] = (idx + offset) as u8;
                    i += 3;
                } else { i += 1; }
            }
            // Jump: target (3 bytes)
            0x50 => {
                i += 3;
            }
            // JumpIfTrue, JumpIfFalse: Rs, target (4 bytes)
            0x51 | 0x52 => {
                i += 4;
            }
            // Binary Ops, Concat: Rd, Rs1, Rs2 (4 bytes)
            0x60 | 0x61 | 0x62 | 0x63 | 0x66 | 0x67 | 0x6A | 0x6B | 0x68 | 0x69 | 0x70 | 0x71 | 0x75 | 0x65 => {
                i += 4;
            }
            // Cast: Rd, Rs, type (4 bytes)
            0x74 => {
                i += 4;
            }
            // Line: line_number (3 bytes)
            0x73 => {
                i += 3;
            }
            // TryStart: catch_pc, catch_reg (4 bytes)
            0x80 => {
                i += 4;
            }
            _ => {
                i += 1;
            }
        }
    }
}

/// Information about a variable's liveness and register allocation
#[derive(Debug, Clone)]
struct VariableLiveness {
    /// Register assigned to this variable
    #[allow(dead_code)] // Used for debugging/inspection
    register: usize,
    /// Whether this is a declared variable (should be preserved for debugger in normal mode)
    is_declared: bool,
    /// Whether the register can be reused (only in unsafe_fast mode)
    can_reuse: bool,
}

/// Liveness analysis result: maps variable names to their last use bytecode position
type LivenessMap = std::collections::HashMap<String, usize>;

/// Perform liveness analysis on statements to find last use of each variable
fn analyze_liveness(stmts: &[crate::parser::Stmt], liveness: &mut LivenessMap, position: &mut usize) {
    for stmt in stmts {
        analyze_stmt_liveness(stmt, liveness, position);
    }
}

fn analyze_stmt_liveness(stmt: &crate::parser::Stmt, liveness: &mut LivenessMap, position: &mut usize) {
    *position += 1; // Each statement gets a position
    match stmt {
        crate::parser::Stmt::Let { name: _, expr, .. } => {
            // The variable is defined here, analyze the expression
            analyze_expr_liveness(expr, liveness, position);
            // Variable 'name' starts its life here (we track uses, not definitions)
        }
        crate::parser::Stmt::Assign { name, expr, .. } => {
            *position += 1;
            liveness.insert(name.clone(), *position);
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::AugAssign { target, expr, .. } => {
            *position += 1;
            match target {
                crate::parser::AugAssignTarget::Variable(name) => {
                    liveness.insert(name.clone(), *position);
                }
                crate::parser::AugAssignTarget::Field { object, name: _ } => {
                    analyze_expr_liveness(object, liveness, position);
                }
            }
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::Return { expr, .. } => {
            *position += 1;
            if let Some(e) = expr {
                analyze_expr_liveness(e, liveness, position);
            }
        }
        crate::parser::Stmt::Expr(expr) => {
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::If { condition, then_branch, else_branch, .. } => {
            analyze_expr_liveness(condition, liveness, position);
            analyze_liveness(then_branch, liveness, position);
            if let Some(else_b) = else_branch {
                analyze_liveness(else_b, liveness, position);
            }
        }
        crate::parser::Stmt::For { var_name, range, body, .. } => {
            if let crate::parser::Expr::Range { start, end, .. } = range.as_ref() {
                analyze_expr_liveness(start, liveness, position);
                analyze_expr_liveness(end, liveness, position);
            }
            // Track uses of loop variable in body
            analyze_liveness_with_var(body, liveness, position, var_name);
        }
        crate::parser::Stmt::While { condition, body, .. } => {
            analyze_expr_liveness(condition, liveness, position);
            analyze_liveness(body, liveness, position);
        }
        crate::parser::Stmt::TryCatch { try_block, catch_var, catch_block, .. } => {
            analyze_liveness(try_block, liveness, position);
            analyze_liveness_with_var(catch_block, liveness, position, catch_var);
        }
        crate::parser::Stmt::Throw { expr, .. } => {
            analyze_expr_liveness(expr, liveness, position);
        }
        // These don't affect local variable liveness
        crate::parser::Stmt::Break(_) 
        | crate::parser::Stmt::Continue(_)
        | crate::parser::Stmt::Module { .. } 
        | crate::parser::Stmt::Import { .. } 
        | crate::parser::Stmt::Class(_) 
        | crate::parser::Stmt::Interface(_) 
        | crate::parser::Stmt::Enum(_) 
        | crate::parser::Stmt::Function(_) 
        | crate::parser::Stmt::TypeAlias(_) => {}
    }
}

fn analyze_liveness_with_var(stmts: &[crate::parser::Stmt], liveness: &mut LivenessMap, position: &mut usize, var: &str) {
    for stmt in stmts {
        analyze_stmt_liveness_with_var(stmt, liveness, position, var);
    }
}

fn analyze_stmt_liveness_with_var(stmt: &crate::parser::Stmt, liveness: &mut LivenessMap, position: &mut usize, var: &str) {
    *position += 1;
    match stmt {
        crate::parser::Stmt::Let { name, expr, .. } => {
            if name == var {
                // Shadowing - stop tracking outer var
                return;
            }
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::Assign { name, expr, .. } => {
            if name == var {
                *position += 1;
                liveness.insert(var.to_string(), *position);
            }
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::AugAssign { target, expr, .. } => {
            match target {
                crate::parser::AugAssignTarget::Variable(name) => {
                    if name == var {
                        *position += 1;
                        liveness.insert(var.to_string(), *position);
                    }
                }
                crate::parser::AugAssignTarget::Field { object, name: _ } => {
                    analyze_expr_liveness(object, liveness, position);
                }
            }
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::Return { expr, .. } => {
            if let Some(e) = expr {
                analyze_expr_liveness(e, liveness, position);
            }
        }
        crate::parser::Stmt::Expr(expr) => {
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::If { condition, then_branch, else_branch, .. } => {
            analyze_expr_liveness(condition, liveness, position);
            analyze_liveness_with_var(then_branch, liveness, position, var);
            if let Some(else_b) = else_branch {
                analyze_liveness_with_var(else_b, liveness, position, var);
            }
        }
        crate::parser::Stmt::For { var_name, range, body, .. } => {
            if var_name == var {
                return; // Shadowing
            }
            if let crate::parser::Expr::Range { start, end, .. } = range.as_ref() {
                analyze_expr_liveness(start, liveness, position);
                analyze_expr_liveness(end, liveness, position);
            }
            analyze_liveness_with_var(body, liveness, position, var);
        }
        crate::parser::Stmt::While { condition, body, .. } => {
            analyze_expr_liveness(condition, liveness, position);
            analyze_liveness_with_var(body, liveness, position, var);
        }
        crate::parser::Stmt::TryCatch { try_block, catch_var, catch_block, .. } => {
            if catch_var == var {
                analyze_liveness(catch_block, liveness, position);
            } else {
                analyze_liveness(try_block, liveness, position);
                analyze_liveness_with_var(catch_block, liveness, position, var);
            }
        }
        crate::parser::Stmt::Throw { expr, .. } => {
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Stmt::Break(_) 
        | crate::parser::Stmt::Continue(_)
        | crate::parser::Stmt::Module { .. } 
        | crate::parser::Stmt::Import { .. } 
        | crate::parser::Stmt::Class(_) 
        | crate::parser::Stmt::Interface(_) 
        | crate::parser::Stmt::Enum(_) 
        | crate::parser::Stmt::Function(_) 
        | crate::parser::Stmt::TypeAlias(_) => {}
    }
}

fn analyze_expr_liveness(expr: &crate::parser::Expr, liveness: &mut LivenessMap, position: &mut usize) {
    *position += 1;
    match expr {
        crate::parser::Expr::Variable { name, .. } => {
            // Record use of this variable
            liveness.insert(name.clone(), *position);
        }
        crate::parser::Expr::Literal(_) => {}
        crate::parser::Expr::Binary { left, right, .. } => {
            analyze_expr_liveness(left, liveness, position);
            analyze_expr_liveness(right, liveness, position);
        }
        crate::parser::Expr::Unary { expr: operand, .. } => {
            analyze_expr_liveness(operand, liveness, position);
        }
        crate::parser::Expr::Call { callee, args, .. } => {
            analyze_expr_liveness(callee, liveness, position);
            for arg in args {
                analyze_expr_liveness(arg, liveness, position);
            }
        }
        crate::parser::Expr::Get { object, .. } => {
            analyze_expr_liveness(object, liveness, position);
        }
        crate::parser::Expr::Set { object, value, .. } => {
            analyze_expr_liveness(object, liveness, position);
            analyze_expr_liveness(value, liveness, position);
        }
        crate::parser::Expr::Range { start, end, .. } => {
            analyze_expr_liveness(start, liveness, position);
            analyze_expr_liveness(end, liveness, position);
        }
        crate::parser::Expr::Array { elements, .. } => {
            for elem in elements {
                analyze_expr_liveness(elem, liveness, position);
            }
        }
        crate::parser::Expr::ObjectLiteral { fields, .. } => {
            for field in fields {
                analyze_expr_liveness(&field.value, liveness, position);
            }
        }
        crate::parser::Expr::Interpolated { parts, .. } => {
            for part in parts {
                if let crate::parser::InterpPart::Expr(e) = part {
                    analyze_expr_liveness(e, liveness, position);
                }
            }
        }
        crate::parser::Expr::Cast { expr, .. } => {
            analyze_expr_liveness(expr, liveness, position);
        }
        crate::parser::Expr::Index { object, index, .. } => {
            analyze_expr_liveness(object, liveness, position);
            analyze_expr_liveness(index, liveness, position);
        }
        crate::parser::Expr::Lambda { body, .. } => {
            // For lambdas, analyze the body but variables inside are local to the lambda
            analyze_liveness(body, liveness, position);
        }
    }
}

/// Compilation context for a single function/method
struct CompileContext {
    /// Next available register
    next_reg: usize,
    /// Maximum register used (for determining frame size)
    max_reg: usize,
    /// Local variable name to register mapping
    locals_map: std::collections::HashMap<String, usize>,
    /// Parameter names (for register assignment)
    params: Vec<String>,
    /// Variable liveness tracking (only used in unsafe_fast mode)
    variable_liveness: std::collections::HashMap<String, VariableLiveness>,
    /// Stack of reusable registers (freed registers that can be reused)
    reusable_regs: Vec<usize>,
    /// Whether we're in unsafe_fast mode (enables register reuse)
    unsafe_fast: bool,
    /// Liveness analysis: maps variable names to their last use position
    liveness_map: LivenessMap,
    /// Current bytecode position during compilation
    current_position: usize,
}

impl CompileContext {
    fn new() -> Self {
        Self {
            next_reg: 1, // R0 is reserved for return value
            max_reg: 0,
            locals_map: std::collections::HashMap::new(),
            params: Vec::new(),
            variable_liveness: std::collections::HashMap::new(),
            reusable_regs: Vec::new(),
            unsafe_fast: false,
            liveness_map: LivenessMap::new(),
            current_position: 0,
        }
    }

    fn with_params(params: Vec<String>) -> Self {
        let mut ctx = Self::new();
        // Assign parameters to R1, R2, ..., Rn
        for (i, param) in params.iter().enumerate() {
            ctx.locals_map.insert(param.clone(), i + 1);
            // Parameters are declared variables
            ctx.variable_liveness.insert(param.clone(), VariableLiveness {
                register: i + 1,
                is_declared: true,
                can_reuse: false, // Parameters can't be reused until end of function
            });
        }
        ctx.next_reg = params.len() + 1;
        ctx.max_reg = params.len();
        ctx.params = params;
        ctx
    }

    fn new_with_unsafe_fast(unsafe_fast: bool) -> Self {
        let mut ctx = Self::new();
        ctx.unsafe_fast = unsafe_fast;
        ctx
    }

    fn with_params_and_unsafe_fast(params: Vec<String>, unsafe_fast: bool) -> Self {
        let mut ctx = Self::with_params(params);
        ctx.unsafe_fast = unsafe_fast;
        ctx
    }

    /// Set liveness map from analysis
    fn set_liveness_map(&mut self, liveness: LivenessMap) {
        self.liveness_map = liveness;
    }

    /// Increment current position and return it
    #[allow(dead_code)] // Reserved for future liveness improvements
    fn advance_position(&mut self) -> usize {
        self.current_position += 1;
        self.current_position
    }

    /// Get current position
    #[allow(dead_code)] // Reserved for future liveness improvements
    fn get_position(&self) -> usize {
        self.current_position
    }

    /// Check if a variable use at current position is its last use
    #[allow(dead_code)] // Reserved for future liveness improvements
    fn is_last_use(&self, name: &str) -> bool {
        if !self.unsafe_fast {
            return false;
        }
        self.liveness_map.get(name).copied() == Some(self.current_position)
    }

    /// Release a variable's register if it's no longer needed
    fn release_if_last_use(&mut self, name: &str) {
        if !self.unsafe_fast {
            return;
        }
        if let Some(liveness) = self.variable_liveness.get_mut(name) {
            if self.liveness_map.get(name).copied() == Some(self.current_position) {
                self.reusable_regs.push(liveness.register);
                liveness.can_reuse = true;
            }
        }
    }

    fn allocate_reg(&mut self) -> usize {
        // In unsafe_fast mode, try to reuse registers first
        if self.unsafe_fast {
            if let Some(reg) = self.reusable_regs.pop() {
                return reg;
            }
        }
        
        let reg = self.next_reg;
        self.next_reg += 1;
        if reg > self.max_reg {
            self.max_reg = reg;
        }
        reg
    }

    fn allocate_regs(&mut self, count: usize) -> usize {
        if count == 0 { return self.next_reg; }
        
        // In unsafe_fast mode, try to reuse consecutive registers if available
        if self.unsafe_fast && self.reusable_regs.len() >= count {
            // Sort reusable regs to get consecutive ones if possible
            self.reusable_regs.sort();
            let start = self.reusable_regs[self.reusable_regs.len() - count];
            for _ in 0..count {
                self.reusable_regs.pop();
            }
            return start;
        }
        
        let start = self.next_reg;
        self.next_reg += count;
        if start + count - 1 > self.max_reg {
            self.max_reg = start + count - 1;
        }
        start
    }

    fn get_local_reg(&mut self, name: &str) -> usize {
        if let Some(&reg) = self.locals_map.get(name) {
            reg
        } else {
            let reg = self.allocate_reg();
            self.locals_map.insert(name.to_string(), reg);
            reg
        }
    }

    /// Declare a new variable with a register, tracking liveness
    fn declare_variable(&mut self, name: String, unsafe_fast: bool) -> usize {
        let reg = self.allocate_reg();
        self.locals_map.insert(name.clone(), reg);
        
        if unsafe_fast {
            self.variable_liveness.insert(name, VariableLiveness {
                register: reg,
                is_declared: true,
                can_reuse: false, // Initially cannot reuse until we know it's last use
            });
        }
        
        reg
    }

    /// Get register for a variable, releasing it after last use in unsafe_fast mode
    fn get_local_reg_and_maybe_release(&mut self, name: &str) -> usize {
        let reg = self.get_local_reg(name);
        self.advance_position();
        self.release_if_last_use(name);
        reg
    }

    /// Mark a variable as no longer needed, allowing register reuse in unsafe_fast mode
    #[allow(dead_code)] // Used for future liveness improvements
    fn release_variable(&mut self, name: &str) {
        if !self.unsafe_fast {
            return; // Don't release in normal mode - keep for debugger
        }
        
        if let Some(liveness) = self.variable_liveness.get_mut(name) {
            if liveness.is_declared {
                // For declared variables, only release if explicitly marked as reusable
                if liveness.can_reuse {
                    self.reusable_regs.push(liveness.register);
                }
            } else {
                // For temporary variables, always release
                self.reusable_regs.push(liveness.register);
            }
        }
    }

    /// Mark a declared variable as eligible for register reuse
    fn mark_variable_reusable(&mut self, name: &str) {
        if !self.unsafe_fast {
            return;
        }
        
        if let Some(liveness) = self.variable_liveness.get_mut(name) {
            if liveness.is_declared {
                liveness.can_reuse = true;
            }
        }
    }

    /// Get total register count needed for this frame (including R0)
    fn register_count(&self) -> u8 {
        (self.max_reg + 1) as u8 // +1 for R0
    }
}

pub struct Compiler {
    source: String,
    _source_path: Option<String>,
    _type_context: Option<TypeContext>,
    break_jumps: Vec<Vec<usize>>,
    continue_targets: Vec<usize>,
    current_ctx: CompileContext,
    /// Track global (module-level) variable names
    global_vars: std::collections::HashSet<String>,
    pub unsafe_fast: bool,
    pub enable_type_checking: bool,
    pub search_paths: Vec<String>,
    /// Stack of scopes, each containing variables declared in that scope
    scope_stack: Vec<Vec<String>>,
    /// Store default expressions for constructor parameters by class name
    constructor_defaults: std::collections::HashMap<String, Vec<Option<crate::parser::Expr>>>,
    /// Store default expressions for function parameters by function name
    function_defaults: std::collections::HashMap<String, Vec<Option<crate::parser::Expr>>>,
}

pub struct CompilerOptions {
    pub enable_type_checking: bool,
    pub search_paths: Vec<String>,
    pub unsafe_fast: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_type_checking: true,
            search_paths: vec!["std".to_string()],
            unsafe_fast: false,
        }
    }
}

impl Default for Compiler {
    fn default() -> Self {
        Self {
            source: String::new(),
            _source_path: None,
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new_with_unsafe_fast(false),
            global_vars: std::collections::HashSet::new(),
            unsafe_fast: false,
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            scope_stack: Vec::new(),
            constructor_defaults: std::collections::HashMap::new(),
            function_defaults: std::collections::HashMap::new(),
        }
    }
}

impl Compiler {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            _source_path: None,
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new_with_unsafe_fast(false),
            global_vars: std::collections::HashSet::new(),
            unsafe_fast: false,
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            scope_stack: Vec::new(),
            constructor_defaults: std::collections::HashMap::new(),
            function_defaults: std::collections::HashMap::new(),
        }
    }

    pub fn with_path(source: &str, path: &str) -> Self {
        Self {
            source: source.to_string(),
            _source_path: Some(path.to_string()),
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new_with_unsafe_fast(false),
            global_vars: std::collections::HashSet::new(),
            unsafe_fast: false,
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            scope_stack: Vec::new(),
            constructor_defaults: std::collections::HashMap::new(),
            function_defaults: std::collections::HashMap::new(),
        }
    }

    pub fn with_options(source: &str, unsafe_fast: bool) -> Self {
        Self {
            source: source.to_string(),
            _source_path: None,
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new_with_unsafe_fast(unsafe_fast),
            global_vars: std::collections::HashSet::new(),
            unsafe_fast,
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            scope_stack: Vec::new(),
            constructor_defaults: std::collections::HashMap::new(),
            function_defaults: std::collections::HashMap::new(),
        }
    }

    pub fn with_path_and_options(source: &str, path: &str, unsafe_fast: bool) -> Self {
        Self {
            source: source.to_string(),
            _source_path: Some(path.to_string()),
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new_with_unsafe_fast(unsafe_fast),
            global_vars: std::collections::HashSet::new(),
            unsafe_fast,
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            scope_stack: Vec::new(),
            constructor_defaults: std::collections::HashMap::new(),
            function_defaults: std::collections::HashMap::new(),
        }
    }


    pub fn compile(&mut self) -> Result<Bytecode, String> {
        let options = CompilerOptions {
            unsafe_fast: self.unsafe_fast,
            ..CompilerOptions::default()
        };
        self.compile_with_options(&options)
    }

    pub fn compile_with_options(&mut self, options: &CompilerOptions) -> Result<Bytecode, String> {
        self.unsafe_fast = options.unsafe_fast;
        
        let mut lexer = Lexer::new(&self.source, self._source_path.as_deref().unwrap_or("unknown"));
        let (tokens, token_positions) = lexer.tokenize()?;

        let mut parser = Parser::new(tokens, &self.source, self._source_path.as_deref().unwrap_or("unknown"), token_positions);
        let statements = parser.parse()?;

        let mut type_context = None;
        let mut resolver = None;

        if options.enable_type_checking {
            let mut resolver_instance = ModuleResolver::new();
            resolver_instance.enable_type_checking = options.enable_type_checking;

            for path in &options.search_paths {
                if let Ok(full_path) = std::path::PathBuf::from(path).canonicalize() {
                    resolver_instance.add_search_path(full_path);
                } else {
                    // If it doesn't exist, try relative to current directory without canonicalize
                    resolver_instance.add_search_path(std::path::PathBuf::from(path));
                }
            }

            match resolver_instance.build_type_context_with_source(&statements, &self.source, self._source_path.as_deref()) {
                Ok(ctx) => {
                    // Check if there were any type errors
                    if ctx.has_errors() {
                        let mut error_msg = String::new();
                        for error in ctx.get_errors() {
                            error_msg.push_str(&format!("{}\n", error.message));
                        }
                        return Err(format!("Type checking failed:\n{}", error_msg));
                    }
                    type_context = Some(ctx.clone());
                    resolver = Some(resolver_instance);
                }
                Err(e) => {
                    return Err(format!("Type checking failed:\n{}", e));
                }
            }
        }

        self.generate_code(&statements, type_context, resolver)
    }

    fn generate_code(&mut self, statements: &[Stmt], type_context: Option<TypeContext>, resolver: Option<ModuleResolver>) -> Result<Bytecode, String> {
        let mut bytecode = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut classes: Vec<ClassDef> = Vec::new();
        let mut functions: Vec<FunctionDef> = Vec::new();

        let mut function_source_files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut function_sources: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut class_source_files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut class_sources: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Clear global variables set for this compilation unit
        self.global_vars.clear();

        // First pass: collect all global variable names (from module-level let statements)
        // This is needed so that static field initializers and other early-compiled code
        // can correctly reference global variables
        for stmt in statements {
            if let Stmt::Let { name, .. } = stmt {
                self.global_vars.insert(name.clone());
            }
        }

        // Collect functions and classes from imported modules first
        if let Some(res) = &resolver {
            for (module_name, module_info) in res.get_loaded_modules() {
                for stmt in &module_info.statements {
                    if let Stmt::Function(func) = stmt {
                        let mut func_with_module = func.clone();
                        let full_name = format!("{}.{}", module_name, func.name);
                        func_with_module.name = full_name.clone();
                        functions.push(func_with_module);
                        function_source_files.insert(full_name.clone(), module_info.path.to_string_lossy().to_string());
                        function_sources.insert(full_name, module_info.source.clone());
                    } else if let Stmt::Class(class) = stmt {
                        let mut class_with_module = class.clone();
                        let full_name = format!("{}.{}", module_name, class.name);
                        class_with_module.name = full_name.clone();
                        classes.push(class_with_module);
                        class_source_files.insert(full_name.clone(), module_info.path.to_string_lossy().to_string());
                        class_sources.insert(full_name, module_info.source.clone());
                    }
                }
            }
        }

        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    classes.push(class.clone());
                }
                Stmt::Interface(interface) => {
                    // Interfaces are compiled into special classes with only methods
                    let interface_as_class = ClassDef {
                        name: interface.name.clone(),
                        type_params: interface.type_params.clone(),
                        parent_interfaces: interface.parent_interfaces.clone(),
                        fields: vec![],
                        methods: interface.methods.clone(),
                        nested_classes: interface.nested_classes.clone(),
                        nested_interfaces: interface.nested_interfaces.clone(),
                        is_native: false,
                        private: false,
                    };

                    classes.push(interface_as_class);
                }
                Stmt::Function(func) => {
                    // Store function default expressions for call site injection
                    if !func.params.is_empty() {
                        let defaults: Vec<Option<crate::parser::Expr>> = func.params.iter()
                            .map(|p| p.default.clone())
                            .collect();
                        self.function_defaults.insert(func.name.clone(), defaults);
                    }
                    functions.push(func.clone());
                }
                _ => {}
            }
        }

        // Compile classes with methods
        let mut vm_classes = Vec::new();
        let mut static_methods = Vec::new();  // Collect static methods to add to global functions
        let mut static_field_initializers: Vec<(String, String, Expr)> = Vec::new();  // (class_name, field_name, default_expr)
        for c in &classes {
            let mut fields = std::collections::HashMap::new();
            let mut private_fields = std::collections::HashSet::new();
            // Initialize all fields to Null - actual initialization happens at module level for static fields
            // and in constructor for instance fields
            for field in &c.fields {
                // Skip static fields - they are stored as module-level variables, not in instances
                if field.is_static {
                    if let Some(default_expr) = &field.default {
                        static_field_initializers.push((c.name.clone(), field.name.clone(), default_expr.clone()));
                    }
                    continue;
                }
                fields.insert(field.name.clone(), Value::Null);
                // Track private fields for reflection/stringification
                if field.private {
                    private_fields.insert(field.name.clone());
                }
            }

            let mut vm_methods = std::collections::HashMap::new();
            let class_source = class_sources.get(&c.name).unwrap_or(&self.source);
            let class_source_file = class_source_files.get(&c.name).cloned().or_else(|| self._source_path.clone());

            for method in &c.methods {
                // Store constructor default expressions for call site injection
                if method.name == "constructor" && !method.params.is_empty() {
                    let defaults: Vec<Option<crate::parser::Expr>> = method.params.iter()
                        .map(|p| p.default.clone())
                        .collect();
                    self.constructor_defaults.insert(c.name.clone(), defaults);
                }

                let mut method_bytecode = Vec::new();
                // Use global strings table directly to avoid index adjustment issues

                let mut method_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
                method_ctx.current_class = Some(c.name.clone());
                method_ctx.current_method_params = method.params.iter().map(|p| p.name.clone()).collect();

                let is_native_method = method.is_native && method.body.is_empty();

                if is_native_method {
                    // For native methods without body, we don't generate bytecode.
                    // The VM will check class.native_methods.
                    continue;
                }

                // Static methods don't need "self" parameter
                let mut method_params = if method.is_static {
                    Vec::new()
                } else {
                    vec!["self".to_string()]
                };
                method_params.extend(method.params.iter().map(|p| p.name.clone()));
                let mut method_compiler = if let Some(path) = &class_source_file {
                    Compiler::with_path_and_options(class_source, path, self.unsafe_fast)
                } else {
                    Compiler::with_options(class_source, self.unsafe_fast)
                };
                // Copy global variables to method compiler so it can reference them
                method_compiler.global_vars = self.global_vars.clone();
                method_compiler.current_ctx = CompileContext::with_params_and_unsafe_fast(method_params, self.unsafe_fast);

                // Run liveness analysis for unsafe_fast mode
                if self.unsafe_fast {
                    let mut liveness_map = LivenessMap::new();
                    let mut position = 0;
                    analyze_liveness(&method.body, &mut liveness_map, &mut position);
                    method_compiler.current_ctx.set_liveness_map(liveness_map);
                }

                // Check if this is an auto-generated constructor (empty body, name is "constructor")
                let is_auto_constructor = method.name == "constructor" && method.body.is_empty();

                if is_auto_constructor {
                    // Generate field assignment code for auto-generated constructor
                    if method.params.is_empty() {
                        // Empty constructor: assign default values from field definitions
                        // Note: Static fields are initialized at module level, not in constructors
                        for field in &c.fields {
                            if let Some(default_expr) = &field.default {
                                // Skip static fields - they are initialized at module level
                                if field.is_static {
                                    continue;
                                }

                                let r_self = method_compiler.current_ctx.get_local_reg("self");
                                let r_val = method_compiler.compile_expr(default_expr, &mut method_bytecode, &mut strings, &classes, Some(&method_ctx))?;

                                let field_name_idx = add_string(&mut strings, field.name.clone());
                                method_bytecode.push(Opcode::SetProperty as u8);
                                method_bytecode.push(r_self as u8);
                                method_bytecode.push(field_name_idx as u8);
                                method_bytecode.push(r_val as u8);
                            }
                        }
                    } else {
                        // Constructor with parameters: assign parameter values to fields
                        for param in &method.params {
                            let r_self = method_compiler.current_ctx.get_local_reg("self");
                            let r_param = method_compiler.current_ctx.get_local_reg(&param.name);

                            let field_name_idx = add_string(&mut strings, param.name.clone());
                            method_bytecode.push(Opcode::SetProperty as u8);
                            method_bytecode.push(r_self as u8);
                            method_bytecode.push(field_name_idx as u8);
                            method_bytecode.push(r_param as u8);
                        }
                    }
                } else if method.name == "constructor" {
                    // Custom constructor: initialize instance fields with defaults first, then run custom body
                    // Note: Static fields are initialized at module level, not in constructors
                    for field in &c.fields {
                        if let Some(default_expr) = &field.default {
                            // Skip static fields - they are initialized at module level
                            if field.is_static {
                                continue;
                            }

                            let r_self = method_compiler.current_ctx.get_local_reg("self");
                            let r_val = method_compiler.compile_expr(default_expr, &mut method_bytecode, &mut strings, &classes, Some(&method_ctx))?;

                            let field_name_idx = add_string(&mut strings, field.name.clone());
                            method_bytecode.push(Opcode::SetProperty as u8);
                            method_bytecode.push(r_self as u8);
                            method_bytecode.push(field_name_idx as u8);
                            method_bytecode.push(r_val as u8);
                        }
                    }

                    // Now compile the custom constructor body
                    for stmt in &method.body {
                        method_compiler.compile_stmt(stmt, &mut method_bytecode, &mut strings, &classes, Some(&method_ctx))?;
                    }
                } else {
                    // Regular method (not a constructor) - just compile the body
                    for stmt in &method.body {
                        method_compiler.compile_stmt(stmt, &mut method_bytecode, &mut strings, &classes, Some(&method_ctx))?;
                    }
                }

                // Ensure method returns null if no explicit return
                // Return is a 2-byte instruction: [opcode, register]
                let ends_with_return = method_bytecode.len() >= 2 &&
                    method_bytecode[method_bytecode.len() - 2] == Opcode::Return as u8;

                if !ends_with_return {
                    // Constructors should return self (R1), not null
                    if method.name == "constructor" {
                        method_bytecode.push(Opcode::Move as u8);
                        method_bytecode.push(0); // R0 (return register)
                        method_bytecode.push(1); // R1 (self)
                        method_bytecode.push(Opcode::Return as u8);
                        method_bytecode.push(0); // R0
                    } else {
                        method_bytecode.push(Opcode::LoadNull as u8);
                        method_bytecode.push(0); // R0
                        method_bytecode.push(Opcode::Return as u8);
                        method_bytecode.push(0); // R0
                    }
                }

                let register_count = method_compiler.current_ctx.register_count();

                // No string index adjustment needed - we used the global strings table directly

                // Generate mangled name for method overloading support based on parameter types
                let mangled_name = if method.params.is_empty() {
                    format!("{}()", method.name)
                } else {
                    // Build a mangled name with actual parameter types
                    let mut params = Vec::new();
                    for param in &method.params {
                        let param_type = param.type_name.as_ref().map(|t| t.clone()).unwrap_or_else(|| "T".to_string());
                        params.push(param_type);
                    }
                    format!("{}({})", method.name, params.join(","))
                };

                // For static methods, also add to global functions list with ClassName::methodName
                if method.is_static {
                    let static_method_name = format!("{}::{}", c.name, mangled_name);
                    static_methods.push((static_method_name, method_bytecode.clone(), register_count, method.params.len()));
                }

                vm_methods.insert(mangled_name.clone(), Method {
                    name: mangled_name.clone(),
                    bytecode: method_bytecode,
                    register_count,
                });
            }

            vm_classes.push(Class {
                name: c.name.clone(),
                fields,
                private_fields,
                methods: vm_methods,
                native_methods: std::collections::HashMap::new(),
                native_create: None,
                native_destroy: None,
                is_native: c.is_native,
                parent_interfaces: c.parent_interfaces.clone(),
                vtable: c.methods.iter().map(|m| m.name.clone()).collect(),
                is_interface: c.fields.is_empty() && c.parent_interfaces.is_empty(),
            });
        }

        // Compile user-defined functions
        let mut vm_functions = Vec::new();
        for f in &functions {
            let mut func_bytecode = Vec::new();
            // Use the global strings table directly to avoid index adjustment issues

            let mut func_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
            func_ctx.current_method_params = f.params.iter().map(|p| p.name.clone()).collect();

            // Set current_module based on the function's qualified name
            // Functions from modules have names like "module.submodule.funcName"
            // We need to extract the module path (everything before the last dot)
            if let Some(last_dot) = f.name.rfind('.') {
                let module_path = &f.name[..last_dot];
                // Only set current_module if it looks like a module path (contains a dot or is a known module)
                if module_path.contains('.') || module_path.starts_with("std") {
                    func_ctx.current_module = Some(module_path.to_string());
                }
            }

            let func_source = function_sources.get(&f.name).unwrap_or(&self.source);
            let mut func_compiler = Compiler::with_options(func_source, self.unsafe_fast);
            // Copy global variables to function compiler so it can reference them
            func_compiler.global_vars = self.global_vars.clone();
            func_compiler.current_ctx = CompileContext::with_params_and_unsafe_fast(f.params.iter().map(|p| p.name.clone()).collect(), self.unsafe_fast);

            // Run liveness analysis for unsafe_fast mode
            if self.unsafe_fast {
                let mut liveness_map = LivenessMap::new();
                let mut position = 0;
                analyze_liveness(&f.body, &mut liveness_map, &mut position);
                func_compiler.current_ctx.set_liveness_map(liveness_map);
            }

            for stmt in &f.body {
                func_compiler.compile_stmt(stmt, &mut func_bytecode, &mut strings, &classes, Some(&func_ctx))?;
            }

            // Check if bytecode already ends with a Return instruction
            // Return is a 2-byte instruction: [opcode, register]
            let ends_with_return = func_bytecode.len() >= 2 &&
                func_bytecode[func_bytecode.len() - 2] == Opcode::Return as u8;

            if !ends_with_return {
                func_bytecode.push(Opcode::LoadNull as u8);
                func_bytecode.push(0); // R0
                func_bytecode.push(Opcode::Return as u8);
                func_bytecode.push(0); // R0
            }

            let source_file = function_source_files.get(&f.name).cloned();
            let register_count = func_compiler.current_ctx.register_count();

            // No string index adjustment needed - we used the global strings table directly

            // Use mangled name from type context if available (for overloaded functions)
            let function_name = if let Some(ctx) = &type_context {
                // Try to find the mangled name for this function
                let param_types: Vec<crate::types::Type> = f.params.iter()
                    .filter_map(|p| p.type_name.as_ref().map(|t| crate::types::Type::from_str(t)))
                    .collect();
                let mangled = crate::types::mangle_function_name(&f.name, &param_types);
                
                // Check if this mangled name exists in the context
                if ctx.functions.contains_key(&mangled) {
                    mangled
                } else {
                    f.name.clone()
                }
            } else {
                f.name.clone()
            };

            vm_functions.push(Function {
                name: function_name,
                bytecode: func_bytecode,
                param_count: f.params.len() as u8,
                register_count,
                source_file,
            });
        }

        // Add static methods to global functions
        // Note: Static method bytecode has already been adjusted and strings extended during class compilation
        for (name, bytecode, register_count, param_count) in static_methods {
            vm_functions.push(Function {
                name,
                bytecode,
                param_count: param_count as u8,
                register_count,
                source_file: None,
            });
        }

        // Initialize static fields at module level before any other code runs
        self.current_ctx = CompileContext::new_with_unsafe_fast(self.unsafe_fast);
        for (class_name, field_name, default_expr) in &static_field_initializers {
            let r_val = self.compile_expr(default_expr, &mut bytecode, &mut strings, &classes, type_context.as_ref())?;
            let idx = add_string(&mut strings, format!("static_{}.{}", class_name, field_name));
            bytecode.push(Opcode::StoreLocal as u8);
            bytecode.push(idx as u8);
            bytecode.push(r_val as u8);
        }

        // Compile module-level statements
        self.current_ctx = CompileContext::new_with_unsafe_fast(self.unsafe_fast);
        
        // Run liveness analysis for unsafe_fast mode
        if self.unsafe_fast {
            let mut liveness_map = LivenessMap::new();
            let mut position = 0;
            analyze_liveness(statements, &mut liveness_map, &mut position);
            self.current_ctx.set_liveness_map(liveness_map);
        }
        
        for stmt in statements {
            self.compile_stmt(stmt, &mut bytecode, &mut strings, &classes, type_context.as_ref())?;
        }

        bytecode.push(Opcode::Halt as u8);

        // Build vtables for interface dispatch
        // Each class that implements an interface gets a vtable entry
        let mut vtables = Vec::new();
        for class_def in &vm_classes {
            if !class_def.parent_interfaces.is_empty() || class_def.is_interface {
                // This class implements interfaces - create a vtable
                let vtable_methods: Vec<String> = class_def.vtable.iter()
                    .map(|method_name| {
                        // Extract base method name (without parameter signature)
                        if let Some(paren_pos) = method_name.find('(') {
                            method_name[..paren_pos].to_string()
                        } else {
                            method_name.clone()
                        }
                    })
                    .collect();
                
                vtables.push(sparkler::executor::VTable {
                    class_name: class_def.name.clone(),
                    methods: vtable_methods,
                });
            }
        }

        Ok(Bytecode {
            data: bytecode,
            strings,
            classes: vm_classes,
            functions: vm_functions,
            vtables,
        })
    }

    fn emit_jump(&self, opcode: Opcode, bytecode: &mut Vec<u8>) -> usize {
        bytecode.push(opcode as u8);
        let pos = bytecode.len();
        bytecode.push(0);
        bytecode.push(0);
        pos
    }

    fn patch_jump(&self, pos: usize, target: usize, bytecode: &mut Vec<u8>) {
        let bytes = (target as u16).to_le_bytes();
        bytecode[pos] = bytes[0];
        bytecode[pos + 1] = bytes[1];
    }

    /// Enter a new scope (for block statements, loops, etc.)
    fn enter_scope(&mut self) {
        self.scope_stack.push(Vec::new());
    }

    /// Exit the current scope, releasing variables for register reuse in unsafe_fast mode
    fn exit_scope(&mut self) {
        if let Some(scope_vars) = self.scope_stack.pop() {
            if self.unsafe_fast {
                for var_name in scope_vars {
                    self.current_ctx.mark_variable_reusable(&var_name);
                }
            }
        }
    }

    /// Track a variable declaration in the current scope
    fn track_var_in_scope(&mut self, name: String) {
        if let Some(scope) = self.scope_stack.last_mut() {
            scope.push(name);
        }
    }

    /// Get type ID for overflow checking: 1: int8, 2: uint8, 3: int16, 4: uint16, 5: int32, 6: uint32, 7: int64, 8: uint64, 0: int
    fn get_type_id_for_var(&self, var_name: &str, type_context: Option<&TypeContext>) -> i64 {
        if let Some(ctx) = type_context {
            if let Some(var_info) = ctx.get_variable(var_name) {
                return self.type_to_id(&var_info.type_name);
            }
        }
        0 // Default to int (no bounds check)
    }

    /// Get type ID for a field: 1: int8, 2: uint8, 3: int16, 4: uint16, 5: int32, 6: uint32, 7: int64, 8: uint64, 0: int
    fn get_type_id_for_field(&self, class_name: &str, field_name: &str, type_context: Option<&TypeContext>) -> i64 {
        if let Some(ctx) = type_context {
            if let Some(class_info) = ctx.get_class(class_name) {
                if let Some(field_info) = class_info.fields.get(field_name) {
                    return self.type_to_id(&field_info.type_name);
                }
            }
        }
        0 // Default to int (no bounds check)
    }

    fn type_to_id(&self, ty: &crate::types::Type) -> i64 {
        match ty {
            crate::types::Type::Int8 => 1,
            crate::types::Type::UInt8 => 2,
            crate::types::Type::Int16 => 3,
            crate::types::Type::UInt16 => 4,
            crate::types::Type::Int32 => 5,
            crate::types::Type::UInt32 => 6,
            crate::types::Type::Int64 => 7,
            crate::types::Type::UInt64 => 8,
            _ => 0, // int, float, or other
        }
    }

    fn emit_overflow_check(&mut self, r_op1: usize, r_op2: usize, r_res: usize, op_type: i64, type_id: i64, bytecode: &mut Vec<u8>, strings: &mut Vec<String>) {
        // We'll use a native function call for overflow checking
        let arg_start = self.current_ctx.allocate_regs(5);

        // Arg 1: Left operand
        bytecode.push(Opcode::Move as u8);
        bytecode.push(arg_start as u8);
        bytecode.push(r_op1 as u8);

        // Arg 2: Right operand
        bytecode.push(Opcode::Move as u8);
        bytecode.push((arg_start + 1) as u8);
        bytecode.push(r_op2 as u8);

        // Arg 3: Result
        bytecode.push(Opcode::Move as u8);
        bytecode.push((arg_start + 2) as u8);
        bytecode.push(r_res as u8);

        // Arg 4: Operation type (0: Add, 1: Sub, 2: Mul)
        bytecode.push(Opcode::LoadInt as u8);
        bytecode.push((arg_start + 3) as u8);
        bytecode.extend_from_slice(&op_type.to_le_bytes());

        // Arg 5: Type ID (1: int8, 2: uint8, 3: int16, 4: uint16, 5: int32, 6: uint32, 7: int64, 8: uint64, 0: int)
        bytecode.push(Opcode::LoadInt as u8);
        bytecode.push((arg_start + 4) as u8);
        bytecode.extend_from_slice(&type_id.to_le_bytes());

        let name_idx = add_string(strings, "std.math.check_overflow".to_string());

        bytecode.push(Opcode::CallNative as u8);
        bytecode.push(0 as u8);        // dummy destination
        bytecode.push(name_idx as u8); // function name index
        bytecode.push(arg_start as u8);// arg_start
        bytecode.push(5u8);            // arg_count: 5 arguments
    }

    fn compile_stmt(&mut self, stmt: &Stmt, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<(), String> {
        let line = self.get_statement_line(stmt);
        bytecode.push(Opcode::Line as u8);
        bytecode.extend_from_slice(&(line as u16).to_le_bytes());

        // Advance position for liveness tracking
        self.current_ctx.advance_position();

        // Track register count before statement for temporary cleanup
        let reg_before = self.current_ctx.next_reg;

        match stmt {
            Stmt::Module { .. } | Stmt::Import { .. } | Stmt::Class(_) | Stmt::Interface(_) | Stmt::Enum(_) | Stmt::Function(_) | Stmt::TypeAlias(_) => {}

            Stmt::Let { name, expr, .. } => {
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                // Check if this is a global (module-level) variable
                // Global variables are those not declared within a function/method context
                // We detect this by checking if we're in a function context (current_method_params would be set)
                let is_global = self.current_ctx.params.is_empty() &&
                    !self.current_ctx.locals_map.contains_key("self");

                if is_global {
                    // Use StoreLocal for global variables (stores in VM's locals HashMap)
                    self.global_vars.insert(name.clone());
                    let idx = add_string(strings, name.clone());
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(idx as u8);
                    bytecode.push(r as u8);
                } else {
                    // Use Move for local variables (stores in registers)
                    // In unsafe_fast mode, use declare_variable to track liveness
                    let rd = self.current_ctx.declare_variable(name.clone(), self.unsafe_fast);
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(rd as u8);
                    bytecode.push(r as u8);
                    // Track variable in current scope for release at scope end
                    self.track_var_in_scope(name.clone());
                }
            }

            Stmt::Assign { name, expr, .. } => {
                let mut handled = false;
                if let Some(ctx) = type_context {
                    if let Some(pos) = ctx.current_method_params.iter().position(|p| p == name) {
                        let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                        let rd = pos + 1; // R1..Rn for params
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(r as u8);
                        handled = true;
                    } else if let Some(current_class_name) = &ctx.current_class {
                        if let Some(class_info) = ctx.get_class(current_class_name) {
                            if class_info.fields.contains_key(name) {
                                let r_self = self.current_ctx.get_local_reg("self");
                                let r_val = self.compile_expr(expr, bytecode, strings, classes, type_context)?;

                                let field_name_idx = add_string(strings, name.clone());
                                bytecode.push(Opcode::SetProperty as u8);
                                bytecode.push(r_self as u8);
                                bytecode.push(field_name_idx as u8);
                                bytecode.push(r_val as u8);
                                handled = true;
                            }
                        }
                    }
                }

                if !handled {
                    let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                    let rd = self.current_ctx.get_local_reg(name);
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(rd as u8);
                    bytecode.push(r as u8);
                }
            }

            Stmt::AugAssign { target, op, expr, .. } => {
                match target {
                    crate::parser::AugAssignTarget::Variable(name) => {
                        // Compile: name += expr  =>  name = name + expr
                        // Check if this is a global (module-level) variable
                        let is_global = self.global_vars.contains(name);
                        
                        if is_global {
                            // For global variables, we need to:
                            // 1. Load the current value from the VM's locals HashMap
                            // 2. Compile the RHS expression
                            // 3. Perform the operation
                            // 4. Store the result back to the VM's locals HashMap
                            
                            // Load current value
                            let r_var = self.current_ctx.allocate_reg();
                            let name_idx = add_string(strings, name.clone());
                            bytecode.push(Opcode::LoadLocal as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(name_idx as u8);
                            
                            // Compile RHS expression
                            let r_expr = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                            
                            // Perform operation
                            let r_temp = self.current_ctx.allocate_reg();
                            match op {
                                crate::parser::AugOp::Add => {
                                    bytecode.push(Opcode::Add as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Subtract => {
                                    bytecode.push(Opcode::Subtract as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Multiply => {
                                    bytecode.push(Opcode::Multiply as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Divide => {
                                    bytecode.push(Opcode::Divide as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Modulo => {
                                    bytecode.push(Opcode::Modulo as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitAnd => {
                                    bytecode.push(Opcode::BitAnd as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitOr => {
                                    bytecode.push(Opcode::BitOr as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitXor => {
                                    bytecode.push(Opcode::BitXor as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::ShiftLeft => {
                                    bytecode.push(Opcode::ShiftLeft as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::ShiftRight => {
                                    bytecode.push(Opcode::ShiftRight as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                            }
                            
                            // Store result back to global variable
                            bytecode.push(Opcode::StoreLocal as u8);
                            bytecode.push(name_idx as u8);
                            bytecode.push(r_temp as u8);
                        } else {
                            // For local variables (in functions), use register-based approach
                            // Get the register for the variable (must already exist)
                            let r_var = self.current_ctx.get_local_reg(name);

                            // Compile the right-hand side expression
                            let r_expr = self.compile_expr(expr, bytecode, strings, classes, type_context)?;

                            // Perform the operation: r_var = r_var op r_expr
                            // We need a temp register for the result
                            let r_temp = self.current_ctx.allocate_reg();
                            match op {
                                crate::parser::AugOp::Add => {
                                    bytecode.push(Opcode::Add as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Subtract => {
                                    bytecode.push(Opcode::Subtract as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Multiply => {
                                    bytecode.push(Opcode::Multiply as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Divide => {
                                    bytecode.push(Opcode::Divide as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::Modulo => {
                                    bytecode.push(Opcode::Modulo as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitAnd => {
                                    bytecode.push(Opcode::BitAnd as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitOr => {
                                    bytecode.push(Opcode::BitOr as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::BitXor => {
                                    bytecode.push(Opcode::BitXor as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::ShiftLeft => {
                                    bytecode.push(Opcode::ShiftLeft as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                                crate::parser::AugOp::ShiftRight => {
                                    bytecode.push(Opcode::ShiftRight as u8);
                                    bytecode.push(r_temp as u8);
                                    bytecode.push(r_var as u8);
                                    bytecode.push(r_expr as u8);
                                }
                            }

                            // Store result back to variable
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(r_temp as u8);
                        }
                    }
                    crate::parser::AugAssignTarget::Field { object, name } => {
                        // Compile: obj.field += expr  =>  obj.field = obj.field + expr
                        // First, compile the object expression
                        let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;

                        // Get the current field value into a register
                        let r_field = self.current_ctx.allocate_reg();
                        let field_name_idx = add_string(strings, name.clone());
                        bytecode.push(Opcode::GetProperty as u8);
                        bytecode.push(r_field as u8);
                        bytecode.push(r_obj as u8);
                        bytecode.push(field_name_idx as u8);

                        // Compile the right-hand side expression
                        let r_expr = self.compile_expr(expr, bytecode, strings, classes, type_context)?;

                        // Perform the operation
                        let r_result = self.current_ctx.allocate_reg();
                        match op {
                            crate::parser::AugOp::Add => {
                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::Subtract => {
                                bytecode.push(Opcode::Subtract as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::Multiply => {
                                bytecode.push(Opcode::Multiply as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::Divide => {
                                bytecode.push(Opcode::Divide as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::Modulo => {
                                bytecode.push(Opcode::Modulo as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::BitAnd => {
                                bytecode.push(Opcode::BitAnd as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::BitOr => {
                                bytecode.push(Opcode::BitOr as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::BitXor => {
                                bytecode.push(Opcode::BitXor as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::ShiftLeft => {
                                bytecode.push(Opcode::ShiftLeft as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                            crate::parser::AugOp::ShiftRight => {
                                bytecode.push(Opcode::ShiftRight as u8);
                                bytecode.push(r_result as u8);
                                bytecode.push(r_field as u8);
                                bytecode.push(r_expr as u8);
                            }
                        }

                        // Store result back to field
                        let field_name_idx = add_string(strings, name.clone());
                        bytecode.push(Opcode::SetProperty as u8);
                        bytecode.push(r_obj as u8);
                        bytecode.push(field_name_idx as u8);
                        bytecode.push(r_result as u8);
                    }
                }
            }

            Stmt::Return { expr, .. } => {
                let r = if let Some(e) = expr {
                    self.compile_expr(e, bytecode, strings, classes, type_context)?
                } else {
                    let rd = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::LoadNull as u8);
                    bytecode.push(rd as u8);
                    rd
                };
                bytecode.push(Opcode::Return as u8);
                bytecode.push(r as u8);
            }

            Stmt::Expr(expr) => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
            }

            Stmt::If { condition, then_branch, else_branch, .. } => {
                let r_cond = self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                bytecode.push(r_cond as u8);
                let else_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);

                // Then branch is a new scope
                self.enter_scope();
                for stmt in then_branch {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }
                self.exit_scope();

                if let Some(else_b) = else_branch {
                    let end_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                    let else_target = bytecode.len();
                    self.patch_jump(else_jump_pos, else_target, bytecode);

                    // Else branch is a new scope
                    self.enter_scope();
                    for stmt in else_b {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }
                    self.exit_scope();

                    let end_target = bytecode.len();
                    self.patch_jump(end_jump_pos, end_target, bytecode);
                } else {
                    let end_target = bytecode.len();
                    self.patch_jump(else_jump_pos, end_target, bytecode);
                }
            }

            Stmt::For { var_name, range, body, .. } => {
                if let Expr::Range { start, end, .. } = range.as_ref() {
                    let r_iter = self.current_ctx.get_local_reg(var_name);
                    let r_start_expr = self.compile_expr(start, bytecode, strings, classes, type_context)?;

                    // Initialize iterator
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_start_expr as u8);

                    // Capture end value to a fresh register
                    let r_end_expr = self.compile_expr(end, bytecode, strings, classes, type_context)?;
                    let r_end = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(r_end as u8);
                    bytecode.push(r_end_expr as u8);

                    // Determine direction: r_is_desc = r_iter > r_end
                    let r_is_desc = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::Greater as u8);
                    bytecode.push(r_is_desc as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_end as u8);

                    let first_check_jump = self.emit_jump(Opcode::Jump, bytecode);

                    // increment_start (continue target)
                    let increment_start = bytecode.len();
                    self.continue_targets.push(increment_start);
                    self.break_jumps.push(Vec::new());

                    bytecode.push(Opcode::JumpIfTrue as u8);
                    bytecode.push(r_is_desc as u8);
                    let desc_step_jump_pos = bytecode.len();
                    bytecode.push(0); bytecode.push(0);

                    // Increasing: r_iter++
                    let r_one = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::LoadInt as u8);
                    bytecode.push(r_one as u8);
                    bytecode.extend_from_slice(&1i64.to_le_bytes());
                    bytecode.push(Opcode::Add as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_one as u8);
                    let step_done_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                    // Descending: r_iter--
                    let desc_step_start = bytecode.len();
                    self.patch_jump(desc_step_jump_pos, desc_step_start, bytecode);
                    let r_minus_one = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::LoadInt as u8);
                    bytecode.push(r_minus_one as u8);
                    bytecode.extend_from_slice(&1i64.to_le_bytes());
                    bytecode.push(Opcode::Subtract as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_minus_one as u8);

                    let step_done_pos = bytecode.len();
                    self.patch_jump(step_done_jump_pos, step_done_pos, bytecode);

                    // check_cond
                    let check_cond_start = bytecode.len();
                    self.patch_jump(first_check_jump, check_cond_start, bytecode);

                    bytecode.push(Opcode::JumpIfTrue as u8);
                    bytecode.push(r_is_desc as u8);
                    let desc_comp_jump_pos = bytecode.len();
                    bytecode.push(0); bytecode.push(0);

                    // Increasing: r_iter <= r_end
                    let r_cond = self.current_ctx.allocate_reg();
                    bytecode.push(Opcode::LessEqual as u8);
                    bytecode.push(r_cond as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_end as u8);
                    let comp_done_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                    // Descending: r_iter >= r_end
                    let desc_comp_start = bytecode.len();
                    self.patch_jump(desc_comp_jump_pos, desc_comp_start, bytecode);
                    bytecode.push(Opcode::GreaterEqual as u8);
                    bytecode.push(r_cond as u8);
                    bytecode.push(r_iter as u8);
                    bytecode.push(r_end as u8);

                    let comp_done_pos = bytecode.len();
                    self.patch_jump(comp_done_jump_pos, comp_done_pos, bytecode);

                    bytecode.push(Opcode::JumpIfFalse as u8);
                    bytecode.push(r_cond as u8);
                    let exit_jump_pos = bytecode.len();
                    bytecode.push(0); bytecode.push(0);

                    // For loop body is a new scope
                    self.enter_scope();
                    // Track the loop variable in this scope
                    self.track_var_in_scope(var_name.clone());
                    for stmt in body {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }
                    self.exit_scope();

                    let jump_back = self.emit_jump(Opcode::Jump, bytecode);
                    self.patch_jump(jump_back, increment_start, bytecode);

                    let exit_pos = bytecode.len();
                    self.patch_jump(exit_jump_pos, exit_pos, bytecode);

                    if let Some(jumps) = self.break_jumps.pop() {
                        for jump_pos in jumps {
                            self.patch_jump(jump_pos, exit_pos, bytecode);
                        }
                    }
                    self.continue_targets.pop();
                }
            }

            Stmt::While { condition, body, .. } => {
                let loop_start = bytecode.len();

                self.continue_targets.push(loop_start);
                self.break_jumps.push(Vec::new());

                let r_cond = self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                bytecode.push(r_cond as u8);
                let exit_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);

                // While loop body is a new scope
                self.enter_scope();
                for stmt in body {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }
                self.exit_scope();

                let jump_back = self.emit_jump(Opcode::Jump, bytecode);
                self.patch_jump(jump_back, loop_start, bytecode);

                let exit_pos = bytecode.len();
                self.patch_jump(exit_jump_pos, exit_pos, bytecode);

                if let Some(jumps) = self.break_jumps.pop() {
                    for jump_pos in jumps {
                        self.patch_jump(jump_pos, exit_pos, bytecode);
                    }
                }
                self.continue_targets.pop();
            }

            Stmt::Break(_) => {
                let jump_pos = self.emit_jump(Opcode::Jump, bytecode);
                if let Some(jumps) = self.break_jumps.last_mut() {
                    jumps.push(jump_pos);
                }
            }

            Stmt::Continue(_) => {
                if let Some(&target) = self.continue_targets.last() {
                    let jump_pos = self.emit_jump(Opcode::Jump, bytecode);
                    self.patch_jump(jump_pos, target, bytecode);
                }
            }

            Stmt::TryCatch { try_block, catch_var, catch_block, .. } => {
                bytecode.push(Opcode::TryStart as u8);
                let catch_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);
                let catch_reg = self.current_ctx.get_local_reg(catch_var);
                bytecode.push(catch_reg as u8);

                // Try block is a new scope
                self.enter_scope();
                self.track_var_in_scope(catch_var.clone());
                for stmt in try_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }
                self.exit_scope();

                bytecode.push(Opcode::TryEnd as u8);
                let end_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                let catch_start = bytecode.len();
                self.patch_jump(catch_jump_pos, catch_start, bytecode);

                // Catch block is a new scope
                self.enter_scope();
                for stmt in catch_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }
                self.exit_scope();

                let end_pos = bytecode.len();
                self.patch_jump(end_jump_pos, end_pos, bytecode);
            }

            Stmt::Throw { expr, .. } => {
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                bytecode.push(Opcode::Throw as u8);
                bytecode.push(r as u8);
            }
        }

        // In unsafe_fast mode, release temporary registers after statement
        if self.unsafe_fast && self.current_ctx.next_reg > reg_before {
            // Release all registers that were allocated during this statement
            // (except those assigned to live variables)
            for reg in reg_before..self.current_ctx.next_reg {
                // Check if this register is assigned to any live variable
                let is_assigned = self.current_ctx.variable_liveness.values().any(|v| v.register == reg && !v.can_reuse);
                if !is_assigned {
                    self.current_ctx.reusable_regs.push(reg);
                }
            }
            // Reset next_reg to allow reuse
            self.current_ctx.next_reg = reg_before;
        }

        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<usize, String> {
        match expr {
            Expr::Literal(lit) => {
                let rd = self.current_ctx.allocate_reg();
                match lit {
                    Literal::String(s, _) => {
                        let idx = add_string(strings, s.clone());
                        bytecode.push(Opcode::LoadConst as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                    }
                    Literal::Int(n, _) => {
                        bytecode.push(Opcode::LoadInt as u8);
                        bytecode.push(rd as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Float(n, _) => {
                        bytecode.push(Opcode::LoadFloat as u8);
                        bytecode.push(rd as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Bool(b, _) => {
                        bytecode.push(Opcode::LoadBool as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(if *b { 1 } else { 0 });
                    }
                    Literal::Null(_) => {
                        bytecode.push(Opcode::LoadNull as u8);
                        bytecode.push(rd as u8);
                    }
                }
                Ok(rd)
            }

            Expr::Variable { name, .. } => {
                if name == "self" {
                    return Ok(self.current_ctx.get_local_reg("self"));
                }

                // Check if it's a local variable or parameter first
                if self.current_ctx.locals_map.contains_key(name) {
                    return Ok(self.current_ctx.get_local_reg_and_maybe_release(name));
                }

                // Check if it's a global (module-level) variable
                if self.global_vars.contains(name) {
                    // Use LoadLocal for global variables (loads from VM's locals HashMap)
                    let rd = self.current_ctx.allocate_reg();
                    let idx = add_string(strings, name.clone());
                    bytecode.push(Opcode::LoadLocal as u8);
                    bytecode.push(rd as u8);
                    bytecode.push(idx as u8);
                    return Ok(rd);
                }

                // Check if it's a known global from the type context (e.g., built-in ARGV)
                if let Some(ctx) = type_context {
                    if ctx.variables.contains_key(name) {
                        // Use LoadLocal for known globals
                        let rd = self.current_ctx.allocate_reg();
                        let idx = add_string(strings, name.clone());
                        bytecode.push(Opcode::LoadLocal as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                        return Ok(rd);
                    }
                }

                if let Some(ctx) = type_context {
                    if let Some(current_class_name) = &ctx.current_class {
                        if let Some(class_info) = ctx.get_class(current_class_name) {
                            if let Some(field_info) = class_info.fields.get(name) {
                                // Static fields are stored as module-level variables with "static_ClassName.fieldName" naming
                                if field_info.is_static {
                                    let rd = self.current_ctx.allocate_reg();
                                    let static_field_name = format!("static_{}.{}", current_class_name, name);
                                    let idx = add_string(strings, static_field_name);
                                    bytecode.push(Opcode::LoadLocal as u8);
                                    bytecode.push(rd as u8);
                                    bytecode.push(idx as u8);
                                    return Ok(rd);
                                } else {
                                    // Instance fields require self
                                    let r_self = self.current_ctx.get_local_reg("self");
                                    let rd = self.current_ctx.allocate_reg();
                                    let field_name_idx = add_string(strings, name.clone());
                                    bytecode.push(Opcode::GetProperty as u8);
                                    bytecode.push(rd as u8);
                                    bytecode.push(r_self as u8);
                                    bytecode.push(field_name_idx as u8);
                                    return Ok(rd);
                                }
                            }
                        }
                    }
                }

                Ok(self.current_ctx.get_local_reg(name))
            }

            Expr::Binary { left, op, right, .. } => {
                // Handle power operator specially - compiles to native std.math.pow call
                if *op == BinaryOp::Pow {
                    let r_left = self.compile_expr(left, bytecode, strings, classes, type_context)?;
                    let r_right = self.compile_expr(right, bytecode, strings, classes, type_context)?;
                    let rd = self.current_ctx.allocate_reg();

                    // Compile as: std.math.pow(base, exponent)
                    // Arguments must be in consecutive registers for CallNative
                    // Move arguments to consecutive registers if needed
                    let arg_start = rd + 1;  // Use registers after return register

                    // Move left operand to arg_start if needed
                    if r_left != arg_start {
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push(arg_start as u8);
                        bytecode.push(r_left as u8);
                    }

                    // Move right operand to arg_start + 1 if needed
                    if r_right != arg_start + 1 {
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push((arg_start + 1) as u8);
                        bytecode.push(r_right as u8);
                    }

                    // Call native function: std.math.pow(base, exponent)
                    let name_idx = add_string(strings, "std.math.pow".to_string());

                    bytecode.push(Opcode::CallNative as u8);
                    bytecode.push(rd as u8);       // destination register
                    bytecode.push(name_idx as u8); // function name index
                    bytecode.push(arg_start as u8);// arg_start: first arg register
                    bytecode.push(2u8);            // arg_count: 2 arguments

                    return Ok(rd);
                }

                let r1 = self.compile_expr(left, bytecode, strings, classes, type_context)?;
                let r2 = self.compile_expr(right, bytecode, strings, classes, type_context)?;

                // Add safety checks for division and modulo (division by zero)
                if !self.unsafe_fast && (*op == BinaryOp::Divide || *op == BinaryOp::Modulo) {
                    let name_idx = add_string(strings, "std.math.check_div_zero".to_string());

                    bytecode.push(Opcode::CallNative as u8);
                    bytecode.push(0u8);            // dummy destination: R0
                    bytecode.push(name_idx as u8); // function name index
                    bytecode.push(r2 as u8);       // arg_start: divisor register
                    bytecode.push(1u8);            // arg_count: 1 argument (divisor)
                }

                let rd = self.current_ctx.allocate_reg();

                let opcode = match op {
                    BinaryOp::Equal => Opcode::Equal,
                    BinaryOp::NotEqual => Opcode::NotEqual,
                    BinaryOp::And => Opcode::And,
                    BinaryOp::Or => Opcode::Or,
                    BinaryOp::Add => Opcode::Add,
                    BinaryOp::Subtract => Opcode::Subtract,
                    BinaryOp::Multiply => Opcode::Multiply,
                    BinaryOp::Divide => Opcode::Divide,
                    BinaryOp::Modulo => Opcode::Modulo,
                    BinaryOp::Greater => Opcode::Greater,
                    BinaryOp::GreaterEqual => Opcode::GreaterEqual,
                    BinaryOp::Less => Opcode::Less,
                    BinaryOp::LessEqual => Opcode::LessEqual,
                    BinaryOp::BitAnd => Opcode::BitAnd,
                    BinaryOp::BitOr => Opcode::BitOr,
                    BinaryOp::BitXor => Opcode::BitXor,
                    BinaryOp::ShiftLeft => Opcode::ShiftLeft,
                    BinaryOp::ShiftRight => Opcode::ShiftRight,
                    BinaryOp::Pow => unreachable!("Pow operator handled separately"),
                };

                bytecode.push(opcode as u8);
                bytecode.push(rd as u8);
                bytecode.push(r1 as u8);
                bytecode.push(r2 as u8);

                // Add overflow checks for arithmetic operations (only for integers)
                // Disabled due to bytecode corruption issue
                /*
                if !self.unsafe_fast && matches!(op, BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply) {
                    // Only add overflow check for integer types, not strings
                    let left_type = self.infer_expr_type(left, type_context.unwrap());
                    let is_integer_type = matches!(left_type, 
                        crate::types::Type::Int8 | crate::types::Type::UInt8 | 
                        crate::types::Type::Int16 | crate::types::Type::UInt16 |
                        crate::types::Type::Int32 | crate::types::Type::UInt32 |
                        crate::types::Type::Int64 | crate::types::Type::UInt64);
                    
                    if is_integer_type {
                        let op_type = match op {
                            BinaryOp::Add => 0i64,
                            BinaryOp::Subtract => 1i64,
                            BinaryOp::Multiply => 2i64,
                            _ => unreachable!(),
                        };
                        let type_id = self.type_to_id(&left_type);
                        self.emit_overflow_check(r1, r2, rd, op_type, type_id, bytecode, strings);
                    }
                }
                */

                Ok(rd)
            }

            Expr::Unary { op, expr, .. } => {
                match op {
                    UnaryOp::Not => {
                        let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                        let rd = self.current_ctx.allocate_reg();
                        bytecode.push(Opcode::Not as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(r as u8);
                        Ok(rd)
                    }
                    UnaryOp::BitNot => {
                        let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                        let rd = self.current_ctx.allocate_reg();
                        bytecode.push(Opcode::BitNot as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(r as u8);
                        Ok(rd)
                    }
                    UnaryOp::Negate => {
                        // Negation: subtract value from zero
                        let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                        let rd = self.current_ctx.allocate_reg();
                        let r_zero = self.current_ctx.allocate_reg();
                        
                        // Load 0 (works for both int and float)
                        bytecode.push(Opcode::LoadInt as u8);
                        bytecode.push(r_zero as u8);
                        bytecode.extend_from_slice(&0i64.to_le_bytes());
                        
                        // Subtract: rd = 0 - r
                        bytecode.push(Opcode::Subtract as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(r_zero as u8);
                        bytecode.push(r as u8);

                        if !self.unsafe_fast {
                            self.emit_overflow_check(r_zero, r, rd, 1, 0, bytecode, strings);
                        }
                        Ok(rd)
                    }
                    UnaryOp::PrefixIncrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            // Check if this is a static field access within a class context
                            let is_static_field = if let Some(ctx) = type_context {
                                if let Some(current_class_name) = &ctx.current_class {
                                    if let Some(class_info) = ctx.get_class(current_class_name) {
                                        class_info.fields.get(name).map(|f| f.is_static).unwrap_or(false)
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            // Check if this is a global (module-level) variable
                            let is_global = self.global_vars.contains(name);

                            if is_static_field {
                                // Static fields are stored as module-level variables with "static_ClassName.fieldName" naming
                                let static_field_name = if let Some(ctx) = type_context {
                                    if let Some(current_class_name) = &ctx.current_class {
                                        format!("static_{}.{}", current_class_name, name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    name.clone()
                                };
                                let name_idx = add_string(strings, static_field_name);

                                // Load current value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);

                                // Save original value for overflow check
                                let r_orig = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_orig as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                // Increment: r_var = r_var + 1
                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(r_orig, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                // Store back to static field
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);

                                // Return new value
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                Ok(rd)
                            } else if is_global {
                                // For global variables, use LoadLocal/StoreLocal
                                let name_idx = add_string(strings, name.clone());

                                // Load current value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);

                                // Save original value for overflow check
                                let r_orig = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_orig as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                // Increment: r_var = r_var + 1
                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(r_orig, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                // Store back to global variable
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);

                                // Return new value
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                Ok(rd)
                            } else {
                                // For local variables, use register-based approach
                                let r_var = self.current_ctx.get_local_reg(name);

                                // Save original value for overflow check
                                let r_orig = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_orig as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(r_orig, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                Ok(rd)
                            }
                        } else if let Expr::Get { object, name, .. } = expr.as_ref() {
                            // For obj.field: load field, increment, store back, return new value
                            let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                            let r_field = self.current_ctx.allocate_reg();
                            let idx = add_string(strings, name.clone());
                            bytecode.push(Opcode::GetProperty as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);

                            // Get the field type from the object's class
                            let type_id = if let Some(ctx) = type_context {
                                let obj_type = self.infer_expr_type(object, ctx);
                                if let crate::types::Type::Class(class_name) = obj_type {
                                    self.get_type_id_for_field(&class_name, name, type_context)
                                } else {
                                    0
                                }
                            } else {
                                0
                            };

                            // Save original value for overflow check
                            let r_orig = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_orig as u8);
                            bytecode.push(r_field as u8);

                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Add as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_one as u8);

                            if !self.unsafe_fast {
                                self.emit_overflow_check(r_orig, r_one, r_field, 0, type_id, bytecode, strings);
                            }

                            // Store back to property
                            bytecode.push(Opcode::SetProperty as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(r_field as u8);

                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_field as u8);
                            Ok(rd)
                        } else {
                            Err("Prefix increment requires a variable or field access".to_string())
                        }
                    }
                    UnaryOp::PrefixDecrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            // Check if this is a global (module-level) variable
                            let is_global = self.global_vars.contains(name);
                            
                            if is_global {
                                // For global variables, use LoadLocal/StoreLocal
                                let name_idx = add_string(strings, name.clone());
                                
                                // Load current value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);
                                
                                // Save original value for overflow check
                                let r_orig = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_orig as u8);
                                bytecode.push(r_var as u8);
                                
                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());
                                
                                // Decrement: r_var = r_var - 1
                                bytecode.push(Opcode::Subtract as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);
                                
                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(r_orig, r_one, r_var, 1, type_id, bytecode, strings);
                                }
                                
                                // Store back to global variable
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);
                                
                                // Return new value
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                Ok(rd)
                            } else {
                                // For local variables, use register-based approach
                                let r_var = self.current_ctx.get_local_reg(name);
                                
                                // Save original value for overflow check
                                let r_orig = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_orig as u8);
                                bytecode.push(r_var as u8);
                                
                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());
                                
                                bytecode.push(Opcode::Subtract as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);
                                
                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(r_orig, r_one, r_var, 1, type_id, bytecode, strings);
                                }
                                
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                Ok(rd)
                            }
                        } else if let Expr::Get { object, name, .. } = expr.as_ref() {
                            // For obj.field: load field, decrement, store back, return new value
                            let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                            let r_field = self.current_ctx.allocate_reg();
                            let idx = add_string(strings, name.clone());
                            bytecode.push(Opcode::GetProperty as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);

                            // Save original value for overflow check
                            let r_orig = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_orig as u8);
                            bytecode.push(r_field as u8);

                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Subtract as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_one as u8);

                            if !self.unsafe_fast {
                                let type_id = if let Some(ctx) = type_context {
                                    let obj_type = self.infer_expr_type(object, ctx);
                                    if let crate::types::Type::Class(class_name) = obj_type {
                                        self.get_type_id_for_field(&class_name, name, type_context)
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                };
                                self.emit_overflow_check(r_orig, r_one, r_field, 1, type_id, bytecode, strings);
                            }

                            // Store back to property
                            bytecode.push(Opcode::SetProperty as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(r_field as u8);

                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_field as u8);
                            Ok(rd)
                        } else {
                            Err("Prefix decrement requires a variable or field access".to_string())
                        }
                    }
                    UnaryOp::PostfixIncrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            // Check if this is a static field access within a class context
                            let is_static_field = if let Some(ctx) = type_context {
                                if let Some(current_class_name) = &ctx.current_class {
                                    if let Some(class_info) = ctx.get_class(current_class_name) {
                                        class_info.fields.get(name).map(|f| f.is_static).unwrap_or(false)
                                    } else {
                                        false
                                    }
                                } else {
                                    false
                                }
                            } else {
                                false
                            };

                            // Check if this is a global (module-level) variable
                            let is_global = self.global_vars.contains(name);

                            if is_static_field {
                                // Static fields are stored as module-level variables with "static_ClassName.fieldName" naming
                                let static_field_name = if let Some(ctx) = type_context {
                                    if let Some(current_class_name) = &ctx.current_class {
                                        format!("static_{}.{}", current_class_name, name)
                                    } else {
                                        name.clone()
                                    }
                                } else {
                                    name.clone()
                                };
                                let name_idx = add_string(strings, static_field_name);

                                // Load current value and save as return value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);

                                // Save original value for return
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                // Increment: r_var = r_var + 1
                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(rd, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                // Store back to static field
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);

                                Ok(rd)
                            } else if is_global {
                                // For global variables, use LoadLocal/StoreLocal
                                let name_idx = add_string(strings, name.clone());

                                // Load current value and save as return value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);

                                // Save original value for return
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                // Increment: r_var = r_var + 1
                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(rd, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                // Store back to global variable
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);

                                Ok(rd)
                            } else {
                                // For local variables, use register-based approach
                                let r_var = self.current_ctx.get_local_reg(name);
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);

                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());

                                bytecode.push(Opcode::Add as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);

                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(rd, r_one, r_var, 0, type_id, bytecode, strings);
                                }

                                Ok(rd)
                            }
                        } else if let Expr::Get { object, name, .. } = expr.as_ref() {
                            // For obj.field: save original value, increment field, return original
                            let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                            let r_field = self.current_ctx.allocate_reg();
                            let idx = add_string(strings, name.clone());
                            bytecode.push(Opcode::GetProperty as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);

                            // Save original value for return
                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_field as u8);

                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Add as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_one as u8);

                            if !self.unsafe_fast {
                                let type_id = if let Some(ctx) = type_context {
                                    let obj_type = self.infer_expr_type(object, ctx);
                                    if let crate::types::Type::Class(class_name) = obj_type {
                                        self.get_type_id_for_field(&class_name, name, type_context)
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                };
                                self.emit_overflow_check(rd, r_one, r_field, 0, type_id, bytecode, strings);
                            }

                            // Store back to property
                            bytecode.push(Opcode::SetProperty as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(r_field as u8);

                            Ok(rd)
                        } else {
                            Err("Postfix increment requires a variable or field access".to_string())
                        }
                    }
                    UnaryOp::PostfixDecrement | UnaryOp::Decrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            // Check if this is a global (module-level) variable
                            let is_global = self.global_vars.contains(name);
                            
                            if is_global {
                                // For global variables, use LoadLocal/StoreLocal
                                let name_idx = add_string(strings, name.clone());
                                
                                // Load current value and save as return value
                                let r_var = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(name_idx as u8);
                                
                                // Save original value for return
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                
                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());
                                
                                // Decrement: r_var = r_var - 1
                                bytecode.push(Opcode::Subtract as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);
                                
                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(rd, r_one, r_var, 1, type_id, bytecode, strings);
                                }
                                
                                // Store back to global variable
                                bytecode.push(Opcode::StoreLocal as u8);
                                bytecode.push(name_idx as u8);
                                bytecode.push(r_var as u8);
                                
                                Ok(rd)
                            } else {
                                // For local variables, use register-based approach
                                let r_var = self.current_ctx.get_local_reg(name);
                                let rd = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_var as u8);
                                
                                let r_one = self.current_ctx.allocate_reg();
                                bytecode.push(Opcode::LoadInt as u8);
                                bytecode.push(r_one as u8);
                                bytecode.extend_from_slice(&1i64.to_le_bytes());
                                
                                bytecode.push(Opcode::Subtract as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_var as u8);
                                bytecode.push(r_one as u8);
                                
                                if !self.unsafe_fast {
                                    let type_id = self.get_type_id_for_var(name, type_context);
                                    self.emit_overflow_check(rd, r_one, r_var, 1, type_id, bytecode, strings);
                                }
                                
                                Ok(rd)
                            }
                        } else if let Expr::Get { object, name, .. } = expr.as_ref() {
                            // For obj.field: save original value, decrement field, return original
                            let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                            let r_field = self.current_ctx.allocate_reg();
                            let idx = add_string(strings, name.clone());
                            bytecode.push(Opcode::GetProperty as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);

                            // Save original value for return
                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_field as u8);

                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Subtract as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_field as u8);
                            bytecode.push(r_one as u8);

                            if !self.unsafe_fast {
                                let type_id = if let Some(ctx) = type_context {
                                    let obj_type = self.infer_expr_type(object, ctx);
                                    if let crate::types::Type::Class(class_name) = obj_type {
                                        self.get_type_id_for_field(&class_name, name, type_context)
                                    } else {
                                        0
                                    }
                                } else {
                                    0
                                };
                                self.emit_overflow_check(rd, r_one, r_field, 1, type_id, bytecode, strings);
                            }

                            // Store back to property
                            bytecode.push(Opcode::SetProperty as u8);
                            bytecode.push(r_obj as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(r_field as u8);

                            Ok(rd)
                        } else {
                            Err("Decrement requires a variable or field access".to_string())
                        }
                    }
                }
            }

            Expr::Call { callee, args, span: _ } => {
                // First, compile all argument expressions and collect their types
                let mut arg_regs = Vec::new();
                let mut arg_types = Vec::new();
                for arg in args {
                    let reg = self.compile_expr(arg, bytecode, strings, classes, type_context)?;
                    arg_regs.push(reg);
                    // Infer argument type from expression
                    let arg_type = if let Some(ctx) = type_context {
                        self.infer_expr_type(arg, ctx)
                    } else {
                        Type::Unknown
                    };
                    arg_types.push(arg_type);
                }

                let rd = self.current_ctx.allocate_reg();

                if let Expr::Variable { name: func_name, .. } = callee.as_ref() {
                    if func_name == "breakpoint" {
                        bytecode.push(Opcode::Breakpoint as u8);
                        bytecode.push(Opcode::LoadNull as u8);
                        bytecode.push(rd as u8);
                        return Ok(rd);
                    }

                    // Resolve function/class name if type context is available
                    let mut resolved_name = func_name.clone();
                    let mut is_class = false;
                    let mut is_method = false;
                    let mut is_native = false;
                    if let Some(ctx) = type_context {
                        // For generic class instantiations like Array<T>(args), extract base class name
                        let base_class_name = extract_base_class_name(func_name);
                        
                        // For generic function calls like identity<T>(args), extract base function name
                        let base_func_name = extract_base_class_name(func_name);

                        // Check for function call first if there are arguments (to handle str() function vs str class)
                        if !args.is_empty() {
                            // Try base function name first (for generic functions like identity<T>)
                            if let Some(sig) = ctx.resolve_function_call(base_func_name, &arg_types) {
                                resolved_name = sig.mangled_name.clone().unwrap_or(sig.name.clone());
                                is_native = sig.is_native;
                            } else if let Some(resolved_class) = ctx.resolve_class(base_class_name) {
                                resolved_name = resolved_class;
                                is_class = true;
                            }
                        } else {
                            // No arguments - check class first (for class instantiation without constructor args)
                            if let Some(resolved_class) = ctx.resolve_class(base_class_name) {
                                resolved_name = resolved_class;
                                is_class = true;
                            } else if let Some(sig) = ctx.resolve_function_call(base_func_name, &arg_types) {
                                resolved_name = sig.mangled_name.clone().unwrap_or(sig.name.clone());
                                is_native = sig.is_native;
                            }
                        }

                        // If neither class nor function was found, try fallback resolution
                        if !is_class && !is_native {
                            // Check if this might be a private class from another module
                            // by searching for classes that end with the function name
                            // Only error if the class is from a DIFFERENT module and is private
                            for (class_name, class_info) in &ctx.classes {
                                if class_name.ends_with(&format!(".{}", func_name)) {
                                    // Check if this class is from an imported module
                                    let from_imported_module = ctx.imports.iter().any(|import_entry| {
                                        class_name.starts_with(&format!("{}.", import_entry.module_path))
                                    });

                                    // Check if this class is from the current module
                                    let from_current_module = if let Some(ref current_module) = ctx.current_module {
                                        class_name.starts_with(&format!("{}.", current_module))
                                    } else {
                                        false
                                    };

                                    // Only error if the class is from an imported module but NOT from the current module
                                    if from_imported_module && !from_current_module && class_info.private {
                                        // Found a private class from a different module - generate an error
                                        return Err(format!("Class '{}' is private and cannot be accessed from this module", func_name));
                                    }
                                }
                            }

                            if let Some(current_class) = &ctx.current_class {
                                // Check if it's a method on current class
                                if let Some(class_info) = ctx.get_class(current_class) {
                                    if class_info.methods.contains_key(func_name) {
                                        is_method = true;
                                    }
                                }
                            }
                        }
                    } else {
                        // Fallback: check if it's a known class in classes slice
                        if classes.iter().any(|c| &c.name == func_name) {
                            is_class = true;
                        }
                    }

                    if is_class {
                        // ... (keep class creation logic)
                        // 1. Create instance
                        let idx = add_string(strings, resolved_name.clone());
                        bytecode.push(Opcode::Call as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                        bytecode.push(0); // arg_start (not used for class creation)
                        bytecode.push(0); // arg_count (not used for class creation)

                        // 2. Call constructor if it exists
                        // Check if class has constructor overloads
                        let has_constructor = if let Some(ctx) = type_context {
                            if let Some(class_info) = ctx.get_class(&resolved_name) {
                                class_info.method_overloads.contains_key("constructor")
                            } else { true }
                        } else { true };

                        if has_constructor {
                            // Find the constructor with full parameters to get default values
                            let constructor_params = self.constructor_defaults.get(&resolved_name)
                                .cloned()
                                .unwrap_or_default();

                            // Calculate total args needed (provided + defaults)
                            let total_args = std::cmp::max(args.len(), constructor_params.len());
                            
                            let contiguous_start = self.current_ctx.allocate_regs(total_args + 1);
                            // First arg for Invoke is the object (self)
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(rd as u8);

                            // Copy provided arguments
                            for (i, &r) in arg_regs.iter().enumerate() {
                                let r_arg = contiguous_start + 1 + i;
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_arg as u8);
                                bytecode.push(r as u8);
                            }

                            // Compute default values for missing arguments
                            for i in args.len()..total_args {
                                if i < constructor_params.len() {
                                    if let Some(ref default_expr) = constructor_params[i] {
                                        let r_arg = contiguous_start + 1 + i;
                                        let r_val = self.compile_expr(default_expr, bytecode, strings, classes, type_context)?;
                                        bytecode.push(Opcode::Move as u8);
                                        bytecode.push(r_arg as u8);
                                        bytecode.push(r_val as u8);
                                    }
                                }
                            }

                            // Generate mangled constructor name based on ALL argument types (including defaults)
                            let mangled_ctor = if total_args == 0 {
                                "constructor()".to_string()
                            } else {
                                let mut params = Vec::new();
                                let default_ctx = TypeContext::new();
                                let ctx = type_context.as_ref().map_or(&default_ctx, |v| v);
                                for i in 0..total_args {
                                    let arg_type = if i < args.len() {
                                        self.infer_expr_type(&args[i], ctx)
                                    } else if i < constructor_params.len() {
                                        // Infer type from default expression
                                        if let Some(ref default_expr) = constructor_params[i] {
                                            Type::from_str(&self.infer_expr_type(default_expr, ctx).to_str())
                                        } else {
                                            Type::Unknown
                                        }
                                    } else {
                                        Type::Unknown
                                    };
                                    params.push(arg_type.to_str());
                                }
                                format!("constructor({})", params.join(","))
                            };

                            let constructor_idx = add_string(strings, mangled_ctor);
                            bytecode.push(Opcode::Invoke as u8);
                            let r_unused = self.current_ctx.allocate_reg();
                            bytecode.push(r_unused as u8);
                            bytecode.push(constructor_idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((total_args + 1) as u8);
                        }
                    } else if is_method {
                        // Method call on implicit 'self'
                        let r_self = self.current_ctx.get_local_reg("self");
                        let contiguous_start = self.current_ctx.allocate_regs(args.len() + 1);

                        // First arg for Invoke is the object (self)
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push(contiguous_start as u8);
                        bytecode.push(r_self as u8);

                        for (i, &r) in arg_regs.iter().enumerate() {
                            let r_arg = contiguous_start + 1 + i;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_arg as u8);
                            bytecode.push(r as u8);
                        }

                        // Check if this is an interface method
                        let is_interface_method = if let Some(ctx) = type_context {
                            if let Some(current_class) = &ctx.current_class {
                                if let Some(class_info) = ctx.get_class(current_class) {
                                    class_info.is_interface || !class_info.parent_interfaces.is_empty()
                                } else { false }
                            } else { false }
                        } else { false };

                        if is_interface_method {
                            // For interface calls, use method index in vtable
                            // Find the method's position in the interface's vtable
                            let method_idx = if let Some(ctx) = type_context {
                                if let Some(current_class) = &ctx.current_class {
                                    if let Some(class_info) = ctx.get_class(current_class) {
                                        // Find the method index in the vtable
                                        class_info.vtable.iter().position(|m| {
                                            // Compare base names (without parameter signature)
                                            if let Some(paren_pos) = m.find('(') {
                                                &m[..paren_pos] == func_name
                                            } else {
                                                m == func_name
                                            }
                                        }).unwrap_or(0)
                                    } else { 0 }
                                } else { 0 }
                            } else { 0 };

                            bytecode.push(Opcode::InvokeInterface as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(method_idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((args.len() + 1) as u8);
                        } else {
                            // Regular method call - use mangled method name
                            let mangled_method = if args.is_empty() {
                                format!("{}()", func_name)
                            } else {
                                // Infer types from arguments for proper overload resolution
                                let ctx_for_infer = type_context.map(|tc| tc.clone()).unwrap_or_else(|| TypeContext::new());
                                let mut params = Vec::new();
                                for arg in args {
                                    let arg_type = self.infer_expr_type(arg, &ctx_for_infer);
                                    params.push(arg_type.to_str());
                                }
                                format!("{}({})", func_name, params.join(","))
                            };

                            let idx = add_string(strings, mangled_method);
                            bytecode.push(Opcode::Invoke as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((args.len() + 1) as u8);
                        }
                    } else {
                        // Regular function call - handle default parameters
                        // Get default expressions for this function - try both base name and mangled name
                        let base_func_name = resolved_name.split('(').next().unwrap_or(&resolved_name);
                        let function_params = self.function_defaults.get(base_func_name)
                            .or_else(|| self.function_defaults.get(&resolved_name))
                            .cloned()
                            .unwrap_or_default();

                        // Calculate total args needed (provided + defaults)
                        let total_args = std::cmp::max(args.len(), function_params.len());
                        
                        let contiguous_start = self.current_ctx.allocate_regs(total_args);
                        
                        // Copy provided arguments
                        for (i, &r) in arg_regs.iter().enumerate() {
                            let r_arg = contiguous_start + i;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_arg as u8);
                            bytecode.push(r as u8);
                        }

                        // Compute default values for missing arguments
                        for i in args.len()..total_args {
                            if i < function_params.len() {
                                if let Some(ref default_expr) = function_params[i] {
                                    let r_arg = contiguous_start + i;
                                    let r_val = self.compile_expr(default_expr, bytecode, strings, classes, type_context)?;
                                    bytecode.push(Opcode::Move as u8);
                                    bytecode.push(r_arg as u8);
                                    bytecode.push(r_val as u8);
                                }
                            }
                        }

                        // For native functions, use the resolved name (which includes module prefix for imports)
                        let call_name = resolved_name.clone();
                        let idx = add_string(strings, call_name.clone());

                        // Use CallNative for native functions, Call for bytecode functions
                        if is_native {
                            bytecode.push(Opcode::CallNative as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(total_args as u8);
                        } else {
                            bytecode.push(Opcode::Call as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(total_args as u8);
                        }
                    }
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Check if this is a static method call: ClassName.method()
                    let (is_static_call, obj_name) = if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                        // Check classes list directly for static methods
                        let mut found_static = false;
                        for c in classes {
                            if c.name == *obj_name {
                                // Check if any method with this name is static
                                for method in &c.methods {
                                    if method.name == *name && method.is_static {
                                        found_static = true;
                                        break;
                                    }
                                }
                                break;
                            }
                        }
                        (found_static, obj_name.clone())
                    } else { (false, String::new()) };

                    if is_static_call {
                        // Static method call - no self parameter needed
                        let contiguous_start = self.current_ctx.allocate_regs(args.len());
                        for (i, &r) in arg_regs.iter().enumerate() {
                            let r_arg = contiguous_start + i;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_arg as u8);
                            bytecode.push(r as u8);
                        }

                        // Generate mangled method name based on actual argument types
                        let mangled_method = if args.is_empty() {
                            format!("{}()", name)
                        } else {
                            // Infer types from arguments for proper overload resolution
                            let ctx_for_infer = type_context.map(|tc| tc.clone()).unwrap_or_else(|| TypeContext::new());
                            let mut params = Vec::new();
                            for arg in args {
                                let arg_type = self.infer_expr_type(arg, &ctx_for_infer);
                                params.push(arg_type.to_str());
                            }
                            format!("{}({})", name, params.join(","))
                        };

                        // Static methods are looked up by ClassName::mangled_name
                        let full_method_name = format!("{}::{}", obj_name, mangled_method);
                        let idx = add_string(strings, full_method_name);

                        // Use Call for static methods
                        bytecode.push(Opcode::Call as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                        bytecode.push(contiguous_start as u8);
                        bytecode.push(args.len() as u8);
                    } else {
                        // Instance method call
                        let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                        let contiguous_start = self.current_ctx.allocate_regs(args.len() + 1);

                        // First arg for Invoke is the object (self)
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push(contiguous_start as u8);
                        bytecode.push(r_obj as u8);

                        for (i, &r) in arg_regs.iter().enumerate() {
                            let r_arg = contiguous_start + 1 + i;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_arg as u8);
                            bytecode.push(r as u8);
                        }

                        // Check if this is an interface method call
                        let is_interface_method = if let Some(ctx) = type_context {
                            // Try to get the type of the object
                            if let Expr::Variable { .. } = object.as_ref() {
                                if let Some(current_class) = &ctx.current_class {
                                    if let Some(class_info) = ctx.get_class(current_class) {
                                        class_info.is_interface || !class_info.parent_interfaces.is_empty()
                                    } else { false }
                                } else { false }
                            } else { false }
                        } else { false };

                        if is_interface_method {
                            // For interface calls, use method index in vtable
                            let method_idx = if let Some(ctx) = type_context {
                                if let Some(current_class) = &ctx.current_class {
                                    if let Some(class_info) = ctx.get_class(current_class) {
                                        class_info.vtable.iter().position(|m| {
                                            if let Some(paren_pos) = m.find('(') {
                                                &m[..paren_pos] == name
                                            } else {
                                                m == name
                                            }
                                        }).unwrap_or(0)
                                    } else { 0 }
                                } else { 0 }
                            } else { 0 };

                            bytecode.push(Opcode::InvokeInterface as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(method_idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((args.len() + 1) as u8);
                        } else {
                            // Regular method call - use mangled method name
                            let mangled_method = if args.is_empty() {
                                format!("{}()", name)
                            } else {
                                // Infer types from arguments for proper overload resolution
                                let ctx_for_infer = type_context.map(|tc| tc.clone()).unwrap_or_else(|| TypeContext::new());
                                let mut params = Vec::new();
                                for arg in args {
                                    let arg_type = self.infer_expr_type(arg, &ctx_for_infer);
                                    params.push(arg_type.to_str());
                                }
                                format!("{}({})", name, params.join(","))
                            };

                            let idx = add_string(strings, mangled_method);
                            bytecode.push(Opcode::Invoke as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((args.len() + 1) as u8);
                        }
                    }
                }
                Ok(rd)
            }

            Expr::Get { object, name, .. } => {
                // Check if this is static member access: ClassName.member
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if let Some(ctx) = type_context {
                        // Check if obj_name is a class
                        if ctx.get_class(obj_name).is_some() || ctx.get_interface(obj_name).is_some() {
                            // This is static member access
                            if let Some(class_info) = ctx.get_class(obj_name) {
                                if let Some(field_info) = class_info.fields.get(name) {
                                    if field_info.is_static {
                                        // Load static field value - treat as global variable
                                        let rd = self.current_ctx.allocate_reg();
                                        let idx = add_string(strings, format!("static_{}.{}", obj_name, name));
                                        bytecode.push(Opcode::LoadLocal as u8);
                                        bytecode.push(rd as u8);
                                        bytecode.push(idx as u8);
                                        return Ok(rd);
                                    }
                                }
                            }
                            // For static methods, they'll be handled when the Call is processed
                            // Just return a placeholder for now
                        }
                    }
                }

                // Check for static field access through an instance: instance.staticField
                if let Some(ctx) = type_context {
                    let object_type = self.infer_expr_type(object, ctx);
                    if let Type::Class(class_name) = object_type {
                        if let Some(class_info) = ctx.get_class(&class_name) {
                            if let Some(field_info) = class_info.fields.get(name) {
                                if field_info.is_static {
                                    // Static fields must be accessed through the class name, not an instance
                                    return Err(format!("Static field '{}' on class '{}' must be accessed through the class name, not an instance. Use '{}.{}' instead.", name, class_name, class_name, name));
                                }
                            }
                        }
                    }
                }

                let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();
                let idx = add_string(strings, name.clone());
                bytecode.push(Opcode::GetProperty as u8);
                bytecode.push(rd as u8);
                bytecode.push(r_obj as u8);
                bytecode.push(idx as u8);
                Ok(rd)
            }

            Expr::Set { object, name, value, .. } => {
                // Check if this is static field assignment: ClassName.field = value
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if let Some(ctx) = type_context {
                        if let Some(class_info) = ctx.get_class(obj_name) {
                            if let Some(field_info) = class_info.fields.get(name) {
                                if field_info.is_static {
                                    // Static field assignment - treat as global variable
                                    let r_val = self.compile_expr(value, bytecode, strings, classes, type_context)?;
                                    let idx = add_string(strings, format!("static_{}.{}", obj_name, name));
                                    bytecode.push(Opcode::StoreLocal as u8);
                                    bytecode.push(idx as u8);
                                    bytecode.push(r_val as u8);
                                    return Ok(r_val);
                                }
                            }
                        }
                    }
                }

                // Check for static field assignment through an instance: instance.staticField = value
                if let Some(ctx) = type_context {
                    let object_type = self.infer_expr_type(object, ctx);
                    if let Type::Class(class_name) = object_type {
                        if let Some(class_info) = ctx.get_class(&class_name) {
                            if let Some(field_info) = class_info.fields.get(name) {
                                if field_info.is_static {
                                    // Static fields must be accessed through the class name, not an instance
                                    return Err(format!("Static field '{}' on class '{}' must be accessed through the class name, not an instance. Use '{}.{}' instead.", name, class_name, class_name, name));
                                }
                            }
                        }
                    }
                }

                let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let r_val = self.compile_expr(value, bytecode, strings, classes, type_context)?;
                let idx = add_string(strings, name.clone());
                bytecode.push(Opcode::SetProperty as u8);
                bytecode.push(r_obj as u8);
                bytecode.push(idx as u8);
                bytecode.push(r_val as u8);
                Ok(r_val)
            }

            Expr::Interpolated { parts, .. } => {
                let start_reg = self.current_ctx.allocate_reg();
                // We need to ensure subsequent parts are also in contiguous registers
                for _i in 1..parts.len() {
                    self.current_ctx.allocate_reg();
                }

                for (i, part) in parts.iter().enumerate() {
                    let rd_part = start_reg + i;
                    match part {
                        InterpPart::Text(s) => {
                            let idx = add_string(strings, s.clone());
                            bytecode.push(Opcode::LoadConst as u8);
                            bytecode.push(rd_part as u8);
                            bytecode.push(idx as u8);
                        }
                        InterpPart::Expr(e) => {
                            let r = self.compile_expr(e, bytecode, strings, classes, type_context)?;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd_part as u8);
                            bytecode.push(r as u8);
                        }
                    }
                }
                let rd = self.current_ctx.allocate_reg();
                bytecode.push(Opcode::Concat as u8);
                bytecode.push(rd as u8);
                bytecode.push(start_reg as u8);
                bytecode.push(parts.len() as u8);
                Ok(rd)
            }

            Expr::Array { elements, .. } => {
                let mut el_regs = Vec::new();
                for el in elements {
                    el_regs.push(self.compile_expr(el, bytecode, strings, classes, type_context)?);
                }

                let rd = self.current_ctx.allocate_reg();
                let contiguous_start = self.current_ctx.allocate_regs(elements.len());
                // Move elements to contiguous registers for Opcode::Array
                for (i, &r) in el_regs.iter().enumerate() {
                    let r_arg = contiguous_start + i;
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(r_arg as u8);
                    bytecode.push(r as u8);
                }

                bytecode.push(Opcode::Array as u8);
                bytecode.push(rd as u8);
                bytecode.push(contiguous_start as u8);
                bytecode.push(elements.len() as u8);

                Ok(rd)
            }

            Expr::Index { object, index, .. } => {
                let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let r_idx = self.compile_expr(index, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();

                bytecode.push(Opcode::Index as u8);
                bytecode.push(rd as u8);
                bytecode.push(r_obj as u8);
                bytecode.push(r_idx as u8);
                Ok(rd)
            }

            Expr::Cast { expr, target_type, .. } => {
                // Note: Cast expressions are no longer generated by the parser.
                // All type conversions are now handled via native function calls.
                // This code is kept for backwards compatibility but should not be reached.
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();
                bytecode.push(Opcode::Convert as u8);
                bytecode.push(rd as u8);
                bytecode.push(r as u8);
                match target_type {
                    CastType::Int => bytecode.push(0x01),
                    CastType::Float => bytecode.push(0x02),
                    CastType::Str => bytecode.push(0x03),
                    CastType::Bool => bytecode.push(0x04),
                    CastType::Int8 => bytecode.push(0x05),
                    CastType::UInt8 => bytecode.push(0x06),
                    CastType::Int16 => bytecode.push(0x07),
                    CastType::UInt16 => bytecode.push(0x08),
                    CastType::Int32 => bytecode.push(0x09),
                    CastType::UInt32 => bytecode.push(0x0A),
                    CastType::Int64 => bytecode.push(0x0B),
                    CastType::UInt64 => bytecode.push(0x0C),
                    CastType::Float32 => bytecode.push(0x0D),
                    CastType::Float64 => bytecode.push(0x0E),
                }
                Ok(rd)
            }

            Expr::ObjectLiteral { fields, span, inferred_type } => {
                // Object literal - determine class type from:
                // 1. inferred_type field (for explicit ClassName { fields } syntax)
                // 2. type context (for type-deduced object literals)
                let class_name: Option<String> = if let Some(name) = inferred_type {
                    Some(name.clone())
                } else if let Some(ctx) = type_context {
                    ctx.expr_types.get(&(span.line, span.column)).cloned()
                } else {
                    None
                };

                let class_name = match class_name {
                    Some(name) => name,
                    None => {
                        return Err(format!(
                            "Object literal at line {} cannot determine class type. Use explicit instantiation: ClassName {{ fields }}",
                            span.line
                        ));
                    }
                };

                // Compile as class instantiation: create instance and set fields
                // 1. Create instance using Call opcode
                let rd = self.current_ctx.allocate_reg();
                let idx = add_string(strings, class_name.clone());
                bytecode.push(Opcode::Call as u8);
                bytecode.push(rd as u8);
                bytecode.push(idx as u8);
                bytecode.push(0); // arg_start
                bytecode.push(0); // arg_count
                
                // 2. Call constructor if it exists
                let has_constructor = if let Some(ctx) = type_context {
                    if let Some(class_info) = ctx.get_class(&class_name) {
                        class_info.method_overloads.contains_key("constructor")
                    } else {
                        false
                    }
                } else {
                    false
                };
                
                if has_constructor {
                    let contiguous_start = self.current_ctx.allocate_regs(1); // just self
                    bytecode.push(Opcode::Move as u8);
                    bytecode.push(contiguous_start as u8);
                    bytecode.push(rd as u8);

                    // Use mangled constructor name for empty constructor
                    let constructor_idx = add_string(strings, "constructor()".to_string());
                    bytecode.push(Opcode::Invoke as u8);
                    let r_unused = self.current_ctx.allocate_reg();
                    bytecode.push(r_unused as u8);
                    bytecode.push(constructor_idx as u8);
                    bytecode.push(contiguous_start as u8);
                    bytecode.push(1); // arg_count (only self)
                }

                // 3. Set each field
                for field in fields {
                    let r_value = self.compile_expr(&field.value, bytecode, strings, classes, type_context)?;
                    let r_obj = rd; // The object we just created

                    let field_name_idx = add_string(strings, field.name.clone());

                    // SetProperty format: SetProperty robj idx rs
                    bytecode.push(Opcode::SetProperty as u8);
                    bytecode.push(r_obj as u8);  // robj - object register
                    bytecode.push(field_name_idx as u8);  // idx - field name index
                    bytecode.push(r_value as u8);  // rs - source value register
                }
                
                Ok(rd)
            }

            Expr::Lambda { params: _, return_type: _, body: _, span } => {
                // Lambda compilation - this requires closure support in the VM
                // For now, we provide a helpful error message
                // Full implementation would require:
                // 1. Creating a closure object that captures outer scope variables
                // 2. Generating a function that can be called through the closure
                // 3. VM support for closure invocation

                return Err(format!(
                    "lambda at line {} is not yet supported at runtime. \
                    Lambda parsing and type checking work, but code generation requires VM closure support.",
                    span.line
                ));
            }

            _ => Err(format!("Unsupported expression: {:?}", expr)),
        }
    }

    /// Infer the type of an expression for overload resolution
    fn infer_expr_type(&self, expr: &Expr, ctx: &TypeContext) -> Type {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::String(_, _) => Type::Str,
                    Literal::Int(_, _) => Type::Int,
                    Literal::Float(_, _) => Type::Float,
                    Literal::Bool(_, _) => Type::Bool,
                    Literal::Null(_) => Type::Null,
                }
            }
            Expr::Variable { name, .. } => {
                // Check local variables first
                if let Some(var_info) = ctx.get_variable(name) {
                    return var_info.type_name.clone();
                }
                // Check class fields
                if let Some(current_class_name) = &ctx.current_class {
                    if let Some(class_info) = ctx.get_class(current_class_name) {
                        if let Some(field_info) = class_info.fields.get(name) {
                            return field_info.type_name.clone();
                        }
                    }
                }
                // Check enums
                if ctx.get_enum(name).is_some() {
                    return Type::Enum(name.clone());
                }
                Type::Unknown
            }
            Expr::Binary { left, op, right, .. } => {
                match op {
                    BinaryOp::Equal | BinaryOp::NotEqual |
                    BinaryOp::And | BinaryOp::Or |
                    BinaryOp::Greater | BinaryOp::GreaterEqual |
                    BinaryOp::Less | BinaryOp::LessEqual => Type::Bool,
                    BinaryOp::Pow => Type::Float,  // std.math.pow returns float
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor |
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => {
                        // Bitwise operations return integer types
                        let left_type = self.infer_expr_type(left, ctx);
                        // For shift operators, preserve the left operand's integer type
                        if let Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                           Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 = &left_type {
                            left_type.clone()
                        } else {
                            Type::Int
                        }
                    }
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply |
                    BinaryOp::Divide | BinaryOp::Modulo => {
                        // Try to get type from left and right operands
                        let left_type = self.infer_expr_type(left, ctx);
                        let right_type = self.infer_expr_type(right, ctx);
                        // If both operands have the same specific integer type, preserve it
                        if left_type == right_type {
                            match left_type {
                                Type::Int8 | Type::UInt8 | Type::Int16 | Type::UInt16 |
                                Type::Int32 | Type::UInt32 | Type::Int64 | Type::UInt64 => return left_type.clone(),
                                _ => {}
                            }
                        }
                        // Return the more precise type (float > int)
                        if left_type == Type::Float || right_type == Type::Float {
                            Type::Float
                        } else if left_type != Type::Unknown {
                            left_type
                        } else {
                            right_type
                        }
                    }
                }
            }
            Expr::Unary { op: _, expr: inner, .. } => {
                self.infer_expr_type(inner, ctx)
            }
            Expr::Call { .. } => {
                // Function call return type - would need full resolution
                Type::Unknown
            }
            Expr::Get { object, name, .. } => {
                // Property access - try to get type from class
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if let Some(var_info) = ctx.get_variable(obj_name) {
                        if let Type::Class(class_name) = &var_info.type_name {
                            if let Some(class_info) = ctx.get_class(class_name) {
                                if let Some(field_info) = class_info.fields.get(name) {
                                    return field_info.type_name.clone();
                                }
                            }
                        }
                    }
                }
                Type::Unknown
            }
            Expr::Array { elements, .. } => {
                if let Some(first) = elements.first() {
                    let inner_type = self.infer_expr_type(first, ctx);
                    Type::Array(Box::new(inner_type))
                } else {
                    Type::Array(Box::new(Type::Unknown))
                }
            }
            Expr::Cast { target_type, .. } => {
                match target_type {
                    CastType::Int => Type::Int,
                    CastType::Float => Type::Float,
                    CastType::Str => Type::Str,
                    CastType::Bool => Type::Bool,
                    CastType::Int8 => Type::Int8,
                    CastType::UInt8 => Type::UInt8,
                    CastType::Int16 => Type::Int16,
                    CastType::UInt16 => Type::UInt16,
                    CastType::Int32 => Type::Int32,
                    CastType::UInt32 => Type::UInt32,
                    CastType::Int64 => Type::Int64,
                    CastType::UInt64 => Type::UInt64,
                    CastType::Float32 => Type::Float32,
                    CastType::Float64 => Type::Float64,
                }
            }
            Expr::Lambda { params: _, return_type, body: _, span: _ } => {
                if let Some(ret) = return_type {
                    let ty = Type::from_str(ret);
                    Type::Function(vec![], Box::new(ty))
                } else {
                    Type::Unknown
                }
            }
            Expr::ObjectLiteral { inferred_type, .. } => {
                if let Some(ty) = inferred_type {
                    Type::Class(ty.clone())
                } else {
                    Type::Unknown
                }
            }
            Expr::Interpolated { .. } => Type::Str,
            Expr::Range { .. } => Type::Unknown,
            Expr::Index { .. } => Type::Unknown,
            Expr::Set { .. } => Type::Null,
        }
    }

    fn get_statement_line(&self, stmt: &Stmt) -> usize {
        match stmt {
            Stmt::Module { span, .. } => span.line,
            Stmt::Import { span, .. } => span.line,
            Stmt::Class(class_def) => {
                // Try to get line from first method's first statement
                if let Some(method) = class_def.methods.first() {
                    return method.body.first().map(|s| self.get_stmt_line(s)).unwrap_or(0);
                }
                0
            },
            Stmt::Interface(_) => 0,
            Stmt::Enum(_) => 0,
            Stmt::Function(func) => func.body.first().map(|s| self.get_stmt_line(s)).unwrap_or(0),
            Stmt::TypeAlias(_) => 0,
            Stmt::Let { span, .. } => span.line,
            Stmt::Assign { span, .. } => span.line,
            Stmt::AugAssign { span, .. } => span.line,
            Stmt::Return { span, .. } => span.line,
            Stmt::Expr(expr) => Self::get_expr_line(expr),
            Stmt::If { span, .. } => span.line,
            Stmt::For { span, .. } => span.line,
            Stmt::While { span, .. } => span.line,
            Stmt::Break(span) => span.line,
            Stmt::Continue(span) => span.line,
            Stmt::TryCatch { span, .. } => span.line,
            Stmt::Throw { span, .. } => span.line,
        }
    }

    fn get_expr_line(expr: &Expr) -> usize {
        match expr {
            Expr::Literal(literal) => {
                match literal {
                    Literal::String(_, span) => span.line,
                    Literal::Int(_, span) => span.line,
                    Literal::Float(_, span) => span.line,
                    Literal::Bool(_, span) => span.line,
                    Literal::Null(span) => span.line,
                }
            },
            Expr::Variable { span, .. } => span.line,
            Expr::Binary { span, .. } => span.line,
            Expr::Unary { span, .. } => span.line,
            Expr::Call { span, .. } => span.line,
            Expr::Get { span, .. } => span.line,
            Expr::Set { span, .. } => span.line,
            Expr::Interpolated { span, .. } => span.line,
            Expr::Range { span, .. } => span.line,
            Expr::Cast { span, .. } => span.line,
            Expr::Array { span, .. } => span.line,
            Expr::Index { span, .. } => span.line,
            Expr::ObjectLiteral { span, .. } => span.line,
            Expr::Lambda { span, .. } => span.line,
        }
    }

    fn get_stmt_line(&self, stmt: &Stmt) -> usize {
        self.get_statement_line(stmt)
    }
}

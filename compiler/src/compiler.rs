use crate::parser::{Stmt, Expr, Literal, Parser, ClassDef, FunctionDef, BinaryOp, UnaryOp, InterpPart, CastType};
use crate::lexer::Lexer;
use crate::resolver::ModuleResolver;
use crate::types::{TypeContext, Type, TypeChecker};
use sparkler::vm::{Class, Value, Function, Method};
use sparkler::opcodes::Opcode;

pub type Bytecode = sparkler::executor::Bytecode;

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
            // Await: Rd, Rs (3 bytes)
            0x47 => {
                i += 3;
            }
            _ => {
                i += 1;
            }
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
}

impl CompileContext {
    fn new() -> Self {
        Self {
            next_reg: 1, // R0 is reserved for return value
            max_reg: 0,
            locals_map: std::collections::HashMap::new(),
            params: Vec::new(),
        }
    }

    fn with_params(params: Vec<String>) -> Self {
        let mut ctx = Self::new();
        // Assign parameters to R1, R2, ..., Rn
        for (i, param) in params.iter().enumerate() {
            ctx.locals_map.insert(param.clone(), i + 1);
        }
        ctx.next_reg = params.len() + 1;
        ctx.max_reg = params.len();
        ctx.params = params;
        ctx
    }

    fn allocate_reg(&mut self) -> usize {
        let reg = self.next_reg;
        self.next_reg += 1;
        if reg > self.max_reg {
            self.max_reg = reg;
        }
        reg
    }

    fn allocate_regs(&mut self, count: usize) -> usize {
        if count == 0 { return self.next_reg; }
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
}

pub struct CompilerOptions {
    pub enable_type_checking: bool,
    pub search_paths: Vec<String>,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_type_checking: true,
            search_paths: vec!["std".to_string()],
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
            current_ctx: CompileContext::new(),
        }
    }

    pub fn with_path(source: &str, path: &str) -> Self {
        Self {
            source: source.to_string(),
            _source_path: Some(path.to_string()),
            _type_context: None,
            break_jumps: Vec::new(),
            continue_targets: Vec::new(),
            current_ctx: CompileContext::new(),
        }
    }

    pub fn compile(&mut self) -> Result<Bytecode, String> {
        self.compile_with_options(&CompilerOptions::default())
    }

    pub fn compile_with_options(&mut self, options: &CompilerOptions) -> Result<Bytecode, String> {
        let mut lexer = Lexer::new(&self.source, self._source_path.as_deref().unwrap_or("unknown"));
        let (tokens, token_positions) = lexer.tokenize()?;

        let mut parser = Parser::new(tokens, &self.source, self._source_path.as_deref().unwrap_or("unknown"), token_positions);
        let statements = parser.parse()?;

        let mut resolver = None;
        let mut type_context = None;
        if options.enable_type_checking {
            let mut resolver_instance = ModuleResolver::new();

            for path in &options.search_paths {
                if let Ok(full_path) = std::path::PathBuf::from(path).canonicalize() {
                    resolver_instance.add_search_path(full_path);
                }
            }

            match resolver_instance.build_type_context_with_source(&statements, &self.source, self._source_path.as_deref()) {
                Ok(ctx) => {
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

        // Collect functions and classes from imported modules first
        if let Some(res) = &resolver {
            for (module_name, module_info) in res.get_loaded_modules() {
                for stmt in &module_info.statements {
                    if let Stmt::Function(func) = stmt {
                        let mut func_with_module = func.clone();
                        let full_name = format!("{}::{}", module_name, func.name);
                        func_with_module.name = full_name.clone();
                        functions.push(func_with_module);
                        function_source_files.insert(full_name.clone(), module_info.path.to_string_lossy().to_string());
                        function_sources.insert(full_name, module_info.source.clone());
                    } else if let Stmt::Class(class) = stmt {
                        let mut class_with_module = class.clone();
                        let full_name = format!("{}::{}", module_name, class.name);
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
                        is_native: false,
                    };
                    classes.push(interface_as_class);
                }
                Stmt::Function(func) => {
                    functions.push(func.clone());
                }
                _ => {}
            }
        }

        // Compile classes with methods
        let mut vm_classes = Vec::new();
        for c in &classes {
            let mut fields = std::collections::HashMap::new();
            // ... (rest of field collection)
            for field in &c.fields {
                let value = if let Some(default_expr) = &field.default {
                    match default_expr {
                        Expr::Literal(Literal::String(s)) => Value::String(s.clone()),
                        Expr::Literal(Literal::Int(n)) => Value::Int64(*n),
                        Expr::Literal(Literal::Float(f)) => Value::Float64(*f),
                        Expr::Literal(Literal::Bool(b)) => Value::Bool(*b),
                        Expr::Literal(Literal::Null) => Value::Null,
                        _ => Value::Null,
                    }
                } else {
                    Value::Null
                };
                fields.insert(field.name.clone(), value);
            }

            let mut vm_methods = std::collections::HashMap::new();
            let class_source = class_sources.get(&c.name).unwrap_or(&self.source);
            let class_source_file = class_source_files.get(&c.name).cloned().or_else(|| self._source_path.clone());

            for method in &c.methods {
                let mut method_bytecode = Vec::new();
                let mut method_strings: Vec<String> = Vec::new();

                let mut method_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
                method_ctx.current_class = Some(c.name.clone());
                method_ctx.current_method_params = method.params.iter().map(|p| p.name.clone()).collect();

                let is_native_method = method.is_native && method.body.is_empty();

                if is_native_method {
                    // For native methods without body, we don't generate bytecode.
                    // The VM will check class.native_methods.
                    continue;
                }

                // Create compiler context for method with "self" as first param
                let mut method_params = vec!["self".to_string()];
                method_params.extend(method.params.iter().map(|p| p.name.clone()));
                let mut method_compiler = if let Some(path) = &class_source_file {
                    Compiler::with_path(class_source, path)
                } else {
                    Compiler::new(class_source)
                };
                method_compiler.current_ctx = CompileContext::with_params(method_params);

                for stmt in &method.body {
                    method_compiler.compile_stmt(stmt, &mut method_bytecode, &mut method_strings, &classes, Some(&method_ctx))?;
                }

                // Ensure method returns null if no explicit return
                // Return is a 2-byte instruction: [opcode, register]
                let ends_with_return = method_bytecode.len() >= 2 && 
                    method_bytecode[method_bytecode.len() - 2] == Opcode::Return as u8;
                
                if !ends_with_return {
                    method_bytecode.push(Opcode::LoadNull as u8);
                    method_bytecode.push(0); // R0
                    method_bytecode.push(Opcode::Return as u8);
                    method_bytecode.push(0); // R0
                }

                let register_count = method_compiler.current_ctx.register_count();
                
                // Adjust string indices in method bytecode to match global string table
                let string_offset = strings.len();
                strings.extend(method_strings);
                adjust_string_indices(&mut method_bytecode, string_offset);

                vm_methods.insert(method.name.clone(), Method {
                    name: method.name.clone(),
                    bytecode: method_bytecode,
                    register_count,
                });
            }

            vm_classes.push(Class {
                name: c.name.clone(),
                fields,
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
            let mut func_strings: Vec<String> = Vec::new();

            let mut func_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
            func_ctx.current_method_params = f.params.iter().map(|p| p.name.clone()).collect();

            let func_source = function_sources.get(&f.name).unwrap_or(&self.source);
            let mut func_compiler = Compiler::new(func_source);
            func_compiler.current_ctx = CompileContext::with_params(f.params.iter().map(|p| p.name.clone()).collect());

            for stmt in &f.body {
                func_compiler.compile_stmt(stmt, &mut func_bytecode, &mut func_strings, &classes, Some(&func_ctx))?;
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

            // Adjust string indices in function bytecode to match global string table
            let string_offset = strings.len();
            strings.extend(func_strings);
            adjust_string_indices(&mut func_bytecode, string_offset);

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

        // Compile module-level statements
        self.current_ctx = CompileContext::new();
        for stmt in statements {
            self.compile_stmt(stmt, &mut bytecode, &mut strings, &classes, type_context.as_ref())?;
        }

        bytecode.push(Opcode::Halt as u8);

        Ok(Bytecode {
            data: bytecode,
            strings,
            classes: vm_classes,
            functions: vm_functions,
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

    fn compile_stmt(&mut self, stmt: &Stmt, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<(), String> {
        let line = self.get_statement_line(stmt);
        bytecode.push(Opcode::Line as u8);
        bytecode.extend_from_slice(&(line as u16).to_le_bytes());

        match stmt {
            Stmt::Module { .. } | Stmt::Import { .. } | Stmt::Class(_) | Stmt::Interface(_) | Stmt::Enum(_) | Stmt::Function(_) | Stmt::TypeAlias(_) => {}

            Stmt::Let { name, expr, .. } => {
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.get_local_reg(name);
                bytecode.push(Opcode::Move as u8);
                bytecode.push(rd as u8);
                bytecode.push(r as u8);
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

                                let field_name_idx = strings.len();
                                strings.push(name.clone());
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

            Stmt::Return(expr) => {
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

            Stmt::If { condition, then_branch, else_branch } => {
                let r_cond = self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                bytecode.push(r_cond as u8);
                let else_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);

                for stmt in then_branch {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                if let Some(else_b) = else_branch {
                    let end_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                    let else_target = bytecode.len();
                    self.patch_jump(else_jump_pos, else_target, bytecode);

                    for stmt in else_b {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }

                    let end_target = bytecode.len();
                    self.patch_jump(end_jump_pos, end_target, bytecode);
                } else {
                    let end_target = bytecode.len();
                    self.patch_jump(else_jump_pos, end_target, bytecode);
                }
            }

            Stmt::For { var_name, range, body } => {
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

                    for stmt in body {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }

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

            Stmt::While { condition, body } => {
                let loop_start = bytecode.len();

                self.continue_targets.push(loop_start);
                self.break_jumps.push(Vec::new());

                let r_cond = self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                bytecode.push(r_cond as u8);
                let exit_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);

                for stmt in body {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

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

            Stmt::Break => {
                let jump_pos = self.emit_jump(Opcode::Jump, bytecode);
                if let Some(jumps) = self.break_jumps.last_mut() {
                    jumps.push(jump_pos);
                }
            }

            Stmt::Continue => {
                if let Some(&target) = self.continue_targets.last() {
                    let jump_pos = self.emit_jump(Opcode::Jump, bytecode);
                    self.patch_jump(jump_pos, target, bytecode);
                }
            }

            Stmt::TryCatch { try_block, catch_var, catch_block } => {
                bytecode.push(Opcode::TryStart as u8);
                let catch_jump_pos = bytecode.len();
                bytecode.push(0); bytecode.push(0);
                let catch_reg = self.current_ctx.get_local_reg(catch_var);
                bytecode.push(catch_reg as u8);

                for stmt in try_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                bytecode.push(Opcode::TryEnd as u8);
                let end_jump_pos = self.emit_jump(Opcode::Jump, bytecode);

                let catch_start = bytecode.len();
                self.patch_jump(catch_jump_pos, catch_start, bytecode);

                // Exception is already in the catch register from TryStart
                for stmt in catch_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                let end_pos = bytecode.len();
                self.patch_jump(end_jump_pos, end_pos, bytecode);
            }

            Stmt::Throw(expr) => {
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                bytecode.push(Opcode::Throw as u8);
                bytecode.push(r as u8);
            }
        }

        Ok(())
    }

    fn compile_expr(&mut self, expr: &Expr, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<usize, String> {
        match expr {
            Expr::Literal(lit) => {
                let rd = self.current_ctx.allocate_reg();
                match lit {
                    Literal::String(s) => {
                        let idx = strings.len();
                        strings.push(s.clone());
                        bytecode.push(Opcode::LoadConst as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                    }
                    Literal::Int(n) => {
                        bytecode.push(Opcode::LoadInt as u8);
                        bytecode.push(rd as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Float(n) => {
                        bytecode.push(Opcode::LoadFloat as u8);
                        bytecode.push(rd as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Bool(b) => {
                        bytecode.push(Opcode::LoadBool as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(if *b { 1 } else { 0 });
                    }
                    Literal::Null => {
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

                if let Some(ctx) = type_context {
                    if let Some(current_class_name) = &ctx.current_class {
                        if let Some(class_info) = ctx.get_class(current_class_name) {
                            if class_info.fields.contains_key(name) {
                                let r_self = self.current_ctx.get_local_reg("self");
                                let rd = self.current_ctx.allocate_reg();
                                let field_name_idx = strings.len();
                                strings.push(name.clone());
                                bytecode.push(Opcode::GetProperty as u8);
                                bytecode.push(rd as u8);
                                bytecode.push(r_self as u8);
                                bytecode.push(field_name_idx as u8);
                                return Ok(rd);
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
                    let name_idx = strings.len();
                    strings.push("std.math.pow".to_string());
                    
                    bytecode.push(Opcode::CallNative as u8);
                    bytecode.push(rd as u8);       // destination register
                    bytecode.push(name_idx as u8); // function name index
                    bytecode.push(arg_start as u8);// arg_start: first arg register
                    bytecode.push(2u8);            // arg_count: 2 arguments
                    
                    return Ok(rd);
                }
                
                let r1 = self.compile_expr(left, bytecode, strings, classes, type_context)?;
                let r2 = self.compile_expr(right, bytecode, strings, classes, type_context)?;
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
                    BinaryOp::Pow => unreachable!("Pow operator handled separately"),
                };

                bytecode.push(opcode as u8);
                bytecode.push(rd as u8);
                bytecode.push(r1 as u8);
                bytecode.push(r2 as u8);
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
                    UnaryOp::PrefixIncrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            let r_var = self.current_ctx.get_local_reg(name);
                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Add as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(r_one as u8);

                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_var as u8);
                            Ok(rd)
                        } else {
                            Err("Prefix increment requires a variable".to_string())
                        }
                    }
                    UnaryOp::PrefixDecrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
                            let r_var = self.current_ctx.get_local_reg(name);
                            let r_one = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::LoadInt as u8);
                            bytecode.push(r_one as u8);
                            bytecode.extend_from_slice(&1i64.to_le_bytes());

                            bytecode.push(Opcode::Subtract as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(r_var as u8);
                            bytecode.push(r_one as u8);

                            let rd = self.current_ctx.allocate_reg();
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(r_var as u8);
                            Ok(rd)
                        } else {
                            Err("Prefix decrement requires a variable".to_string())
                        }
                    }
                    UnaryOp::PostfixIncrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
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
                            Ok(rd)
                        } else {
                            Err("Postfix increment requires a variable".to_string())
                        }
                    }
                    UnaryOp::PostfixDecrement | UnaryOp::Decrement => {
                        if let Expr::Variable { name, .. } = expr.as_ref() {
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
                            Ok(rd)
                        } else {
                            Err("Decrement requires a variable".to_string())
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
                        if let Some(resolved_class) = ctx.resolve_class(func_name) {
                            resolved_name = resolved_class;
                            is_class = true;
                        } else if let Some(sig) = ctx.resolve_function_call(func_name, &arg_types) {
                            // Use mangled name for overloaded function resolution
                            resolved_name = sig.mangled_name.clone().unwrap_or(sig.name.clone());
                            is_native = sig.is_native;
                        } else if let Some(current_class) = &ctx.current_class {
                            // Check if it's a method on current class
                            if let Some(class_info) = ctx.get_class(current_class) {
                                if class_info.methods.contains_key(func_name) {
                                    is_method = true;
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
                        let idx = strings.len();
                        strings.push(resolved_name.clone());
                        bytecode.push(Opcode::Call as u8);
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                        bytecode.push(0); // arg_start (not used for class creation)
                        bytecode.push(0); // arg_count (not used for class creation)

                        // 2. Call constructor if it exists
                        // Note: For now we assume a constructor exists if args are provided, 
                        // or we could check the class definition.
                        let has_constructor = if let Some(ctx) = type_context {
                            if let Some(class_info) = ctx.get_class(&resolved_name) {
                                class_info.methods.contains_key("constructor")
                            } else { true }
                        } else { true };

                        if has_constructor {
                            let contiguous_start = self.current_ctx.allocate_regs(args.len() + 1);
                            // First arg for Invoke is the object (self)
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(rd as u8);

                            for (i, &r) in arg_regs.iter().enumerate() {
                                let r_arg = contiguous_start + 1 + i;
                                bytecode.push(Opcode::Move as u8);
                                bytecode.push(r_arg as u8);
                                bytecode.push(r as u8);
                            }

                            let constructor_idx = strings.len();
                            strings.push("constructor".to_string());
                            bytecode.push(Opcode::Invoke as u8);
                            let r_unused = self.current_ctx.allocate_reg();
                            bytecode.push(r_unused as u8);
                            bytecode.push(constructor_idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push((args.len() + 1) as u8);
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

                        let idx = strings.len();
                        strings.push(func_name.clone());
                        
                        // Check if this is an interface method
                        let is_interface_method = if let Some(ctx) = type_context {
                            if let Some(current_class) = &ctx.current_class {
                                if let Some(class_info) = ctx.get_class(current_class) {
                                    class_info.is_interface || !class_info.parent_interfaces.is_empty()
                                } else { false }
                            } else { false }
                        } else { false };
                        
                        if is_interface_method {
                            bytecode.push(Opcode::InvokeInterface as u8);
                        } else {
                            bytecode.push(Opcode::Invoke as u8);
                        }
                        bytecode.push(rd as u8);
                        bytecode.push(idx as u8);
                        bytecode.push(contiguous_start as u8);
                        bytecode.push((args.len() + 1) as u8);
                    } else {
                        // Regular function call
                        let contiguous_start = self.current_ctx.allocate_regs(args.len());
                        for (i, &r) in arg_regs.iter().enumerate() {
                            let r_arg = contiguous_start + i;
                            bytecode.push(Opcode::Move as u8);
                            bytecode.push(r_arg as u8);
                            bytecode.push(r as u8);
                        }

                        let idx = strings.len();
                        strings.push(resolved_name);
                        
                        // Use CallNative for native functions, Call for bytecode functions
                        if is_native {
                            bytecode.push(Opcode::CallNative as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(args.len() as u8);
                        } else {
                            bytecode.push(Opcode::Call as u8);
                            bytecode.push(rd as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(contiguous_start as u8);
                            bytecode.push(args.len() as u8);
                        }
                    }
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
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

                    let idx = strings.len();
                    strings.push(name.clone());
                    
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
                        bytecode.push(Opcode::InvokeInterface as u8);
                    } else {
                        bytecode.push(Opcode::Invoke as u8);
                    }
                    bytecode.push(rd as u8);
                    bytecode.push(idx as u8);
                    bytecode.push(contiguous_start as u8);
                    bytecode.push((args.len() + 1) as u8);
                }
                Ok(rd)
            }

            Expr::Get { object, name, .. } => {
                let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::GetProperty as u8);
                bytecode.push(rd as u8);
                bytecode.push(r_obj as u8);
                bytecode.push(idx as u8);
                Ok(rd)
            }

            Expr::Set { object, name, value, .. } => {
                let r_obj = self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let r_val = self.compile_expr(value, bytecode, strings, classes, type_context)?;
                let idx = strings.len();
                strings.push(name.clone());
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
                            let idx = strings.len();
                            strings.push(s.clone());
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

            Expr::Await { expr, .. } => {
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();
                bytecode.push(Opcode::Await as u8);
                bytecode.push(rd as u8);
                bytecode.push(r as u8);
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

                // If non-POD, call constructor for each element
                for (i, el) in elements.iter().enumerate() {
                    let mut el_type = Type::Unknown;
                    if let Some(ctx) = type_context {
                        let mut checker = TypeChecker::with_context(ctx.clone());
                        el_type = checker.infer_expr(el);
                    }

                    if !el_type.is_pod() {
                        let r_el = el_regs[i];
                        let constructor_idx = strings.len();
                        strings.push("constructor".to_string());
                        
                        let contiguous_call_start = self.current_ctx.allocate_regs(1);
                        bytecode.push(Opcode::Move as u8);
                        bytecode.push(contiguous_call_start as u8);
                        bytecode.push(r_el as u8);

                        bytecode.push(Opcode::Invoke as u8);
                        let r_unused = self.current_ctx.allocate_reg();
                        bytecode.push(r_unused as u8);
                        bytecode.push(constructor_idx as u8);
                        bytecode.push(contiguous_call_start as u8);
                        bytecode.push(1); // arg_count (only self)
                    }
                }

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
                let r = self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                let rd = self.current_ctx.allocate_reg();
                bytecode.push(Opcode::Cast as u8);
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
                let idx = strings.len();
                strings.push(class_name.clone());
                bytecode.push(Opcode::Call as u8);
                bytecode.push(rd as u8);
                bytecode.push(idx as u8);
                bytecode.push(0); // arg_start
                bytecode.push(0); // arg_count
                
                // 2. Call constructor if it exists
                let has_constructor = if let Some(ctx) = type_context {
                    if let Some(class_info) = ctx.get_class(&class_name) {
                        class_info.methods.contains_key("constructor")
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
                    
                    let constructor_idx = strings.len();
                    strings.push("constructor".to_string());
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
                    
                    let field_name_idx = strings.len();
                    strings.push(field.name.clone());
                    
                    // SetProperty format: SetProperty robj idx rs
                    bytecode.push(Opcode::SetProperty as u8);
                    bytecode.push(r_obj as u8);  // robj - object register
                    bytecode.push(field_name_idx as u8);  // idx - field name index
                    bytecode.push(r_value as u8);  // rs - source value register
                }
                
                Ok(rd)
            }

            Expr::Lambda { params: _, return_type: _, body: _, span, is_async } => {
                // Lambda compilation - this requires closure support in the VM
                // For now, we provide a helpful error message
                // Full implementation would require:
                // 1. Creating a closure object that captures outer scope variables
                // 2. Generating a function that can be called through the closure
                // 3. VM support for closure invocation
                // 4. For async lambdas: Promise handling and await support

                let lambda_kind = if *is_async { "async lambda" } else { "lambda" };
                return Err(format!(
                    "{} at line {} is not yet supported at runtime. \
                    Lambda parsing and type checking work, but code generation requires VM closure support.",
                    lambda_kind, span.line
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
                    Literal::String(_) => Type::Str,
                    Literal::Int(_) => Type::Int,
                    Literal::Float(_) => Type::Float,
                    Literal::Bool(_) => Type::Bool,
                    Literal::Null => Type::Null,
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
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply |
                    BinaryOp::Divide | BinaryOp::Modulo => {
                        // Try to get type from left operand
                        let left_type = self.infer_expr_type(left, ctx);
                        if left_type != Type::Unknown {
                            return left_type;
                        }
                        self.infer_expr_type(right, ctx)
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
            Expr::Lambda { params: _, return_type, body: _, span: _, is_async } => {
                if let Some(ret) = return_type {
                    let ty = Type::from_str(ret);
                    if *is_async {
                        Type::Promise(Box::new(ty))
                    } else {
                        Type::Function(vec![], Box::new(ty))
                    }
                } else {
                    Type::Unknown
                }
            }
            Expr::Await { expr, .. } => {
                let inner_type = self.infer_expr_type(expr, ctx);
                if let Type::Promise(inner) = inner_type {
                    *inner
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

    fn get_statement_line(&self, _stmt: &Stmt) -> usize {
        1
    }
}

use crate::parser::{Stmt, Expr, Literal, Parser, ClassDef, FunctionDef, BinaryOp, UnaryOp, InterpPart, CastType};
use crate::lexer::Lexer;
use crate::resolver::ModuleResolver;
use crate::types::TypeContext;
use sparkler::vm::{Class, Value, Opcode, Function};

pub type Bytecode = sparkler::executor::Bytecode;

pub struct Compiler {
    source: String,
    _source_path: Option<String>,
    _type_context: Option<TypeContext>,
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
        }
    }

    pub fn with_path(source: &str, path: &str) -> Self {
        Self {
            source: source.to_string(),
            _source_path: Some(path.to_string()),
            _type_context: None,
        }
    }

    pub fn compile(&self) -> Result<Bytecode, String> {
        self.compile_with_options(&CompilerOptions::default())
    }

    pub fn compile_with_options(&self, options: &CompilerOptions) -> Result<Bytecode, String> {
        let mut lexer = Lexer::new(&self.source);
        let (tokens, token_positions) = lexer.tokenize()?;

        let mut parser = Parser::new(tokens, &self.source, token_positions);
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

    fn generate_code(&self, statements: &[Stmt], type_context: Option<TypeContext>, resolver: Option<ModuleResolver>) -> Result<Bytecode, String> {
        let mut bytecode = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut classes: Vec<ClassDef> = Vec::new();
        let mut functions: Vec<FunctionDef> = Vec::new();

        // Track source files and source content for functions from imported modules
        let mut function_source_files: std::collections::HashMap<String, String> = std::collections::HashMap::new();
        let mut function_sources: std::collections::HashMap<String, String> = std::collections::HashMap::new();

        // Collect functions and classes from imported modules first (with full qualified names)
        if let Some(res) = &resolver {
            for (module_name, module_info) in res.get_loaded_modules() {
                for stmt in &module_info.statements {
                    if let Stmt::Function(func) = stmt {
                        // Create a new function def with the full qualified name
                        let mut func_with_module = func.clone();
                        let full_name = format!("{}::{}", module_name, func.name);
                        func_with_module.name = full_name.clone();
                        functions.push(func_with_module);
                        // Store the source file path for this function
                        function_source_files.insert(full_name.clone(), module_info.path.to_string_lossy().to_string());
                        // Store the source content for line number calculation
                        function_sources.insert(full_name, module_info.source.clone());
                    } else if let Stmt::Class(class) = stmt {
                        // Create a new class def with the full qualified name
                        let mut class_with_module = class.clone();
                        let full_name = format!("{}::{}", module_name, class.name);
                        class_with_module.name = full_name;
                        classes.push(class_with_module);
                    }
                }
            }
        }

        for stmt in statements {
            match stmt {
                Stmt::Class(class) => {
                    classes.push(class.clone());
                }
                Stmt::Function(func) => {
                    functions.push(func.clone());
                }
                _ => {}
            }
        }

        let mut vm_classes = Vec::new();
        for c in &classes {
            let mut fields = std::collections::HashMap::new();
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
            for method in &c.methods {
                let mut method_bytecode = Vec::new();

                // Create a temporary type context for this method if none exists
                let mut method_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
                method_ctx.current_class = Some(c.name.clone());
                method_ctx.current_method_params = method.params.iter().map(|p| p.name.clone()).collect();

                for stmt in &method.body {
                    self.compile_stmt(stmt, &mut method_bytecode, &mut strings, &classes, Some(&method_ctx))?;
                }
                method_bytecode.push(Opcode::Return as u8);

                vm_methods.insert(method.name.clone(), sparkler::vm::Method {
                    name: method.name.clone(),
                    bytecode: method_bytecode,
                });
            }

            vm_classes.push(Class {
                name: c.name.clone(),
                fields,
                methods: vm_methods,
                native_methods: std::collections::HashMap::new(),
            });
        }

        // Compile user-defined functions
        let mut vm_functions = Vec::new();
        for f in &functions {
            let mut func_bytecode = Vec::new();

            // Create a temporary type context for this function
            let mut func_ctx = type_context.clone().unwrap_or_else(|| TypeContext::new());
            func_ctx.current_method_params = f.params.iter().map(|p| p.name.clone()).collect();

            // Use the correct source for line number calculation
            let func_source = function_sources.get(&f.name).unwrap_or(&self.source);
            let func_compiler = Compiler::new(func_source);

            for stmt in &f.body {
                func_compiler.compile_stmt(stmt, &mut func_bytecode, &mut strings, &classes, Some(&func_ctx))?;
            }
            func_bytecode.push(Opcode::Return as u8);

            // Get the source file for this function (if from an imported module)
            let source_file = function_source_files.get(&f.name).cloned();

            vm_functions.push(Function {
                name: f.name.clone(),
                bytecode: func_bytecode,
                param_count: f.params.len(),
                source_file,
            });
        }

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

    fn compile_stmt(&self, stmt: &Stmt, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<(), String> {
        // Emit line number at the start of each statement
        let line = self.get_statement_line(stmt);
        bytecode.push(Opcode::Line as u8);
        bytecode.push(line as u8);
        
        match stmt {
            Stmt::Module { .. } => {
                // Module declaration is currently a no-op for bytecode generation
                // It can be used for module resolution and namespacing in the future
            }
            Stmt::Import { .. } => {
                // Import handled during type checking
            }
            Stmt::Class(_) => {
                // Class definitions are handled during type checking
            }
            Stmt::Enum(_) => {
                // Enum definitions are handled during type checking
                // Enum variants are accessed at runtime via their integer values
            }
            Stmt::Function(_) => {
                // Function definitions are handled during type checking
                // Runtime function calls are handled via the Call opcode
            }
            Stmt::Let { name, expr } => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                let name_idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::StoreLocal as u8);
                bytecode.push(name_idx as u8);
            }
            Stmt::Assign { name, expr, .. } => {
                let mut handled = false;
                if let Some(ctx) = type_context {
                    // Check if it's a parameter
                    if let Some(pos) = ctx.current_method_params.iter().position(|p| p == name) {
                        self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                        let name_idx = strings.len();
                        strings.push((pos + 1).to_string());
                        bytecode.push(Opcode::StoreLocal as u8);
                        bytecode.push(name_idx as u8);
                        handled = true;
                    } else if let Some(current_class_name) = &ctx.current_class {
                        if let Some(class_info) = ctx.get_class(current_class_name) {
                            if class_info.fields.contains_key(name) {
                                // Field assignment: self.field = expr
                                // Load self (index 0)
                                let self_name_idx = strings.len();
                                strings.push("0".to_string());
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(self_name_idx as u8);
                                
                                // Compile value
                                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                                
                                // SetProperty
                                let field_name_idx = strings.len();
                                strings.push(name.clone());
                                bytecode.push(Opcode::SetProperty as u8);
                                bytecode.push(field_name_idx as u8);
                                handled = true;
                            }
                        }
                    }
                }

                if !handled {
                    self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                    let name_idx = strings.len();
                    strings.push(name.clone());
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(name_idx as u8);
                }
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e, bytecode, strings, classes, type_context)?;
                } else {
                    bytecode.push(Opcode::PushNull as u8);
                }
                bytecode.push(Opcode::Return as u8);
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                bytecode.push(Opcode::Pop as u8);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                let mut else_jump = Vec::new();
                if else_branch.is_some() {
                    bytecode.push(Opcode::JumpIfFalse as u8);
                    else_jump.push(bytecode.len());
                    bytecode.push(0);
                } else {
                    bytecode.push(Opcode::JumpIfFalse as u8);
                    else_jump.push(bytecode.len());
                    bytecode.push(0);
                }

                for stmt in then_branch {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                if let Some(else_b) = else_branch {
                    bytecode.push(Opcode::Jump as u8);
                    let end_jump_pos = bytecode.len();
                    bytecode.push(0);

                    let else_target = bytecode.len();
                    bytecode[else_jump[0]] = (else_target & 0xFF) as u8;

                    for stmt in else_b {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }

                    let end_target = bytecode.len();
                    bytecode[end_jump_pos] = (end_target & 0xFF) as u8;
                } else {
                    let else_target = bytecode.len();
                    bytecode[else_jump[0]] = (else_target & 0xFF) as u8;
                }
            }
            Stmt::For { var_name, range, body } => {
                // Compile range expression
                if let Expr::Range { start, end, .. } = range.as_ref() {
                    // Check if we can determine direction at compile time
                    let is_descending = match (start.as_ref(), end.as_ref()) {
                        (Expr::Literal(Literal::Int(start_val)), Expr::Literal(Literal::Int(end_val))) => {
                            start_val > end_val
                        }
                        _ => false, // Default to ascending for non-literal ranges
                    };

                    // Compile start value
                    self.compile_expr(start, bytecode, strings, classes, type_context)?;

                    // Store as iterator
                    let iter_idx = strings.len();
                    strings.push(format!("__for_iter_{}", var_name));
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(iter_idx as u8);

                    // Compile end value
                    self.compile_expr(end, bytecode, strings, classes, type_context)?;

                    // Store end
                    let end_idx = strings.len();
                    strings.push(format!("__for_end_{}", var_name));
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(end_idx as u8);

                    // Loop start
                    let loop_start = bytecode.len();

                    // Load iterator
                    bytecode.push(Opcode::LoadLocal as u8);
                    bytecode.push(iter_idx as u8);

                    // Load end
                    bytecode.push(Opcode::LoadLocal as u8);
                    bytecode.push(end_idx as u8);

                    // Exit condition depends on direction
                    if is_descending {
                        // For descending: exit when iterator < end
                        bytecode.push(Opcode::JumpIfLess as u8);
                    } else {
                        // For ascending: exit when iterator > end
                        bytecode.push(Opcode::JumpIfGreater as u8);
                    }
                    let exit_jump = bytecode.len();
                    bytecode.push(0);

                    // Store iterator in loop variable
                    bytecode.push(Opcode::LoadLocal as u8);
                    bytecode.push(iter_idx as u8);

                    let var_idx = strings.len();
                    strings.push(var_name.clone());
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(var_idx as u8);

                    // Compile body
                    for stmt in body {
                        self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                    }

                    // Increment/decrement iterator
                    bytecode.push(Opcode::LoadLocal as u8);
                    bytecode.push(iter_idx as u8);

                    bytecode.push(Opcode::PushInt as u8);
                    bytecode.extend_from_slice(&1i64.to_le_bytes());

                    if is_descending {
                        bytecode.push(Opcode::Subtract as u8);
                    } else {
                        bytecode.push(Opcode::Add as u8);
                    }

                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(iter_idx as u8);

                    // Jump back
                    bytecode.push(Opcode::Jump as u8);
                    let jump_back = bytecode.len();
                    bytecode.push(0);

                    // Fix up jumps
                    let exit_pos = bytecode.len();
                    bytecode[exit_jump] = (exit_pos & 0xFF) as u8;
                    bytecode[jump_back] = (loop_start & 0xFF) as u8;
                }
            }
            Stmt::While { condition, body } => {
                let loop_start = bytecode.len();

                // Compile condition
                self.compile_expr(condition, bytecode, strings, classes, type_context)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                let exit_jump = bytecode.len();
                bytecode.push(0); // placeholder

                // Compile body
                for stmt in body {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                // Jump back to start
                bytecode.push(Opcode::Jump as u8);
                bytecode.push((loop_start & 0xFF) as u8);

                // Exit position - fix up jumps
                let exit_pos = bytecode.len();
                bytecode[exit_jump] = (exit_pos & 0xFF) as u8;
            }
            Stmt::TryCatch { try_block, catch_var, catch_block } => {
                bytecode.push(Opcode::TryStart as u8);
                let catch_jump_pos = bytecode.len();
                bytecode.push(0); // placeholder for catch block PC

                for stmt in try_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                bytecode.push(Opcode::TryEnd as u8);
                
                // Jump over catch block after successful try
                bytecode.push(Opcode::Jump as u8);
                let end_jump_pos = bytecode.len();
                bytecode.push(0);

                // Start of catch block
                let catch_start = bytecode.len();
                bytecode[catch_jump_pos] = (catch_start & 0xFF) as u8;

                // Store exception in catch variable
                let var_idx = strings.len();
                strings.push(catch_var.clone());
                bytecode.push(Opcode::StoreLocal as u8);
                bytecode.push(var_idx as u8);

                for stmt in catch_block {
                    self.compile_stmt(stmt, bytecode, strings, classes, type_context)?;
                }

                // End of catch block - fix up jump
                let end_pos = bytecode.len();
                bytecode[end_jump_pos] = (end_pos & 0xFF) as u8;
            }
            Stmt::Throw(expr) => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                bytecode.push(Opcode::Throw as u8);
            }
        }
        Ok(())
    }

    fn compile_expr(&self, expr: &Expr, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef], type_context: Option<&TypeContext>) -> Result<(), String> {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::String(s) => {
                        let idx = strings.len();
                        strings.push(s.clone());
                        bytecode.push(Opcode::PushString as u8);
                        bytecode.push(idx as u8);
                    }
                    Literal::Int(n) => {
                        bytecode.push(Opcode::PushInt as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Float(n) => {
                        bytecode.push(Opcode::PushFloat as u8);
                        bytecode.extend_from_slice(&n.to_le_bytes());
                    }
                    Literal::Bool(b) => {
                        bytecode.push(Opcode::PushBool as u8);
                        bytecode.push(if *b { 1 } else { 0 });
                    }
                    Literal::Null => {
                        bytecode.push(Opcode::PushNull as u8);
                    }
                }
            }
            Expr::Variable { name, .. } => {
                if let Some(ctx) = type_context {
                    if let Some(current_class_name) = &ctx.current_class {
                        if let Some(class_info) = ctx.get_class(current_class_name) {
                            if class_info.fields.contains_key(name) {
                                // Field access: self.field
                                // Load self (index 0)
                                let self_name_idx = strings.len();
                                strings.push("0".to_string());
                                bytecode.push(Opcode::LoadLocal as u8);
                                bytecode.push(self_name_idx as u8);

                                // GetProperty
                                let field_name_idx = strings.len();
                                strings.push(name.clone());
                                bytecode.push(Opcode::GetProperty as u8);
                                bytecode.push(field_name_idx as u8);
                                return Ok(());
                            }
                        }
                    }

                    // Handle parameters and 'self' mapping
                    if let Some(pos) = ctx.current_method_params.iter().position(|p| p == name) {
                        let idx = strings.len();
                        strings.push((pos + 1).to_string()); // Parameters start at index 1
                        bytecode.push(Opcode::LoadLocal as u8);
                        bytecode.push(idx as u8);
                        return Ok(());
                    }

                    if name == "self" {
                        let idx = strings.len();
                        strings.push("0".to_string());
                        bytecode.push(Opcode::LoadLocal as u8);
                        bytecode.push(idx as u8);
                        return Ok(());
                    }
                }

                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::LoadLocal as u8);
                bytecode.push(idx as u8);
            }
            Expr::Binary { left, op, right, .. } => {
                self.compile_expr(left, bytecode, strings, classes, type_context)?;
                self.compile_expr(right, bytecode, strings, classes, type_context)?;

                match op {
                    BinaryOp::Equal => bytecode.push(Opcode::Equal as u8),
                    BinaryOp::NotEqual => {
                        bytecode.push(Opcode::Equal as u8);
                        bytecode.push(Opcode::Not as u8);
                    }
                    BinaryOp::And => bytecode.push(Opcode::And as u8),
                    BinaryOp::Or => bytecode.push(Opcode::Or as u8),
                    BinaryOp::Greater => bytecode.push(Opcode::Greater as u8),
                    BinaryOp::Less => bytecode.push(Opcode::Less as u8),
                    BinaryOp::Add => bytecode.push(Opcode::Add as u8),
                    BinaryOp::Subtract => bytecode.push(Opcode::Subtract as u8),
                    BinaryOp::Multiply => bytecode.push(Opcode::Multiply as u8),
                    BinaryOp::Divide => bytecode.push(Opcode::Divide as u8),
                }
            }
            Expr::Unary { op, expr, .. } => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                match op {
                    UnaryOp::Not => bytecode.push(Opcode::Not as u8),
                }
            }
            Expr::Call { callee, args, .. } => {
                if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Method call: push object first, then args
                    self.compile_expr(object, bytecode, strings, classes, type_context)?;
                    for arg in args {
                        self.compile_expr(arg, bytecode, strings, classes, type_context)?;
                    }

                    let method_idx = strings.len();
                    strings.push(name.clone());
                    bytecode.push(Opcode::Invoke as u8);
                    bytecode.push(method_idx as u8);
                    bytecode.push((args.len() + 1) as u8);
                } else {
                    // Regular function call: push args first, then call
                    for arg in args {
                        self.compile_expr(arg, bytecode, strings, classes, type_context)?;
                    }

                    if let Expr::Variable { name: func_name, .. } = callee.as_ref() {
                        let mut is_native = false;
                        let mut is_async = false;
                        let mut resolved_name = func_name.clone();
                        let mut is_class = false;

                        if let Some(ctx) = type_context {
                            // Use resolve_function to find the fully qualified name
                            if let Some(sig) = ctx.resolve_function(func_name) {
                                is_native = sig.is_native;
                                is_async = sig.is_async;
                                resolved_name = sig.name.clone();
                            } else if let Some(class_name) = ctx.resolve_class(func_name) {
                                // It's a class instantiation
                                is_class = true;
                                resolved_name = class_name;
                            }
                        }

                        if is_class {
                            // Class instantiation - use Call opcode with class name
                            let idx = strings.len();
                            strings.push(resolved_name.clone());
                            bytecode.push(Opcode::Call as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(args.len() as u8);
                        } else if is_native {
                            let idx = strings.len();
                            strings.push(resolved_name.clone());
                            if is_async {
                                bytecode.push(Opcode::CallNativeAsync as u8);
                            } else {
                                bytecode.push(Opcode::CallNative as u8);
                            }
                            bytecode.push(idx as u8);
                            bytecode.push(args.len() as u8);
                        } else if resolved_name.starts_with("C.") {
                            let native_name = resolved_name.strip_prefix("C.").unwrap();
                            let idx = strings.len();
                            strings.push(native_name.to_string());

                            // Check if it's an async native function
                            if native_name == "http_get" || native_name == "http_post" {
                                bytecode.push(Opcode::CallNativeAsync as u8);
                            } else {
                                bytecode.push(Opcode::CallNative as u8);
                            }
                            bytecode.push(idx as u8);
                            bytecode.push(args.len() as u8);
                        } else if resolved_name == "println" || resolved_name == "print" {
                            let idx = strings.len();
                            strings.push(resolved_name.clone());
                            bytecode.push(Opcode::CallNative as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(args.len() as u8);
                        } else {
                            // Check if it's a known function or class in type context
                            let is_defined = if let Some(ctx) = type_context {
                                ctx.resolve_function(func_name).is_some() ||
                                classes.iter().any(|c| c.name == *func_name) ||
                                ctx.get_class(func_name).is_some()
                            } else {
                                false
                            };

                            if !is_defined {
                                return Err(format!("Undefined function: {}", func_name));
                            }

                            let idx = strings.len();
                            strings.push(resolved_name.clone());
                            bytecode.push(Opcode::Call as u8);
                            bytecode.push(idx as u8);
                            bytecode.push(args.len() as u8);
                        }
                    }
                }
            }
            Expr::Get { object, name, .. } => {
                self.compile_expr(object, bytecode, strings, classes, type_context)?;
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::GetProperty as u8);
                bytecode.push(idx as u8);
            }
            Expr::Set { object, name, value, .. } => {
                self.compile_expr(object, bytecode, strings, classes, type_context)?;
                self.compile_expr(value, bytecode, strings, classes, type_context)?;
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::SetProperty as u8);
                bytecode.push(idx as u8);
            }
            Expr::Interpolated { parts, .. } => {
                for part in parts {
                    match part {
                        InterpPart::Text(s) => {
                            let idx = strings.len();
                            strings.push(s.clone());
                            bytecode.push(Opcode::PushString as u8);
                            bytecode.push(idx as u8);
                        }
                        InterpPart::Expr(e) => {
                            self.compile_expr(e, bytecode, strings, classes, type_context)?;
                        }
                    }
                }
                bytecode.push(Opcode::Concat as u8);
                bytecode.push(parts.len() as u8);
            }
            Expr::Range { start: _, end: _, .. } => {
                // Range expressions are only used in for loops and handled specially
                // This should not be reached during normal compilation
                return Err("Range expression outside of for loop".to_string());
            }
            Expr::Await { expr, .. } => {
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                bytecode.push(Opcode::Await as u8);
            }
            Expr::Cast { expr, target_type, .. } => {
                // Compile the inner expression
                self.compile_expr(expr, bytecode, strings, classes, type_context)?;
                
                // Emit Cast opcode with target type
                bytecode.push(Opcode::Cast as u8);
                match target_type {
                    CastType::Int => bytecode.push(0x01),
                    CastType::Float => bytecode.push(0x02),
                    CastType::Str => bytecode.push(0x03),
                    CastType::Bool => bytecode.push(0x04),
                }
            }
        }
        Ok(())
    }

    /// Get approximate line number for a statement by counting newlines in source
    fn get_statement_line(&self, stmt: &Stmt) -> usize {
        // Simple approach: count newlines up to a rough position
        // For better accuracy, we'd need to track positions in the parser
        let source_slice = &self.source;
        let mut line = 1;

        // Match on statement type to find approximate position
        match stmt {
            Stmt::Let { name, .. } => {
                if let Some(pos) = source_slice.find(&format!("let {}", name)) {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::Assign { name, .. } => {
                if let Some(pos) = source_slice.find(&format!("{} =", name)) {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::If { .. } => {
                if let Some(pos) = source_slice.find("if ") {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::Return(_) => {
                if let Some(pos) = source_slice.find("return ") {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::Throw(_) => {
                if let Some(pos) = source_slice.find("throw ") {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::TryCatch { .. } => {
                if let Some(pos) = source_slice.find("try ") {
                    line = source_slice[..pos].matches('\n').count() + 1;
                }
            }
            Stmt::Expr(expr) => {
                // For expression statements, try to find the line by looking for common patterns
                if let Expr::Call { callee, .. } = expr {
                    if let Expr::Variable { name, .. } = callee.as_ref() {
                        // Find all occurrences of the function call pattern
                        let pattern = format!("{}(", name);
                        let mut search_start = 0;
                        while let Some(pos) = source_slice[search_start..].find(&pattern) {
                            let absolute_pos = search_start + pos;
                            // Check if this is a call (not a definition)
                            let before = &source_slice[..absolute_pos];
                            // Skip if it's a function definition (ends with "fn ")
                            if !before.trim_end().ends_with("fn") {
                                line = source_slice[..absolute_pos].matches('\n').count() + 1;
                                break;
                            }
                            search_start = absolute_pos + 1;
                        }
                    }
                }
            }
            _ => {}
        }

        line
    }
}


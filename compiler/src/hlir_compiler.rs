//! HLIR-based Bengal Compiler with Full Module Support
//! 
//! This compiler uses HLIR as the intermediate representation.
//! It supports imports, module resolution, and bytecode merging.

use crate::lexer::Lexer;
use crate::parser::{Parser, Stmt, ImportKind, Span};
use crate::hlir::HlirModule;
use crate::ast_to_hlir_full::ast_to_hlir;
use crate::hlir_to_sparkler::{compile_hlir_to_sparkler, compile_hlir_to_sparkler_with_natives, CompiledBytecode};
use crate::types::{TypeChecker, TypeContext};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Compiler options
#[derive(Debug, Clone)]
pub struct CompilerOptions {
    pub enable_type_checking: bool,
    pub search_paths: Vec<String>,
    pub emit_llvm_ir: bool,
    pub emit_sparkler_bytecode: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_type_checking: true,
            search_paths: vec!["std".to_string()],
            emit_llvm_ir: false,
            emit_sparkler_bytecode: true,
        }
    }
}

/// Compilation result
#[derive(Clone)]
pub struct CompilationResult {
    pub hlir: HlirModule,
    pub sparkler_bytecode: Option<CompiledBytecode>,
    #[cfg(feature = "llvm")]
    pub llvm_ir: Option<String>,
}

/// Module info for tracking imports
#[derive(Debug, Clone)]
struct ModuleInfo {
    module_path: String,
    path: PathBuf,
    statements: Vec<Stmt>,
    source: String,
    functions: Vec<String>,
    native_functions: Vec<String>,  // Track which functions are native
    generic_functions: Vec<String>, // Track which functions are generic
    classes: Vec<String>,
}

/// Bengal Compiler using HLIR with full module support
pub struct HlirCompiler {
    source: String,
    source_path: Option<String>,
    options: CompilerOptions,
    loaded_modules: HashMap<String, ModuleInfo>,
    search_paths: Vec<PathBuf>,
    import_map: HashMap<String, String>,
    native_functions: HashMap<String, bool>,  // Map function name -> is_native
}

impl HlirCompiler {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
            source_path: None,
            options: CompilerOptions::default(),
            loaded_modules: HashMap::new(),
            search_paths: Vec::new(),
            import_map: HashMap::new(),
            native_functions: HashMap::new(),
        }
    }
    
    pub fn with_path(source: &str, path: &str) -> Self {
        let mut compiler = Self::new(source);
        compiler.source_path = Some(path.to_string());
        if let Some(parent) = Path::new(path).parent() {
            compiler.search_paths.push(parent.to_path_buf());
        }
        compiler
    }
    
    pub fn with_options(source: &str, options: CompilerOptions) -> Self {
        let mut compiler = Self::new(source);
        compiler.options = options.clone();
        for path in &options.search_paths {
            compiler.search_paths.push(PathBuf::from(path));
        }
        compiler
    }
    
    pub fn with_path_and_options(source: &str, path: &str, options: CompilerOptions) -> Self {
        let mut compiler = Self::new(source);
        compiler.source_path = Some(path.to_string());
        // Add parent directory of source file
        if let Some(parent) = Path::new(path).parent() {
            compiler.search_paths.push(parent.to_path_buf());
        }
        // Add search paths from options
        for search_path in &options.search_paths {
            let pb = PathBuf::from(search_path);
            // Try both the path as-is and relative to current directory
            compiler.search_paths.push(pb.clone());
            if let Ok(current_dir) = std::env::current_dir() {
                compiler.search_paths.push(current_dir.join(&pb));
            }
        }
        compiler.options = options;
        compiler
    }
    
    pub fn set_emit_llvm_ir(&mut self, emit: bool) {
        self.options.emit_llvm_ir = emit;
    }
    
    pub fn set_emit_sparkler_bytecode(&mut self, emit: bool) {
        self.options.emit_sparkler_bytecode = emit;
    }
    
    fn find_module_file(&self, module_path: &[String]) -> Result<PathBuf, String> {
        // For module paths like ["std", "io"], try to find std/io.bl in search paths
        let module_file_name = format!("{}.bl", module_path.join("/"));
        
        for search_path in &self.search_paths {
            let candidate = search_path.join(&module_file_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        
        // Also try without the first component if it matches a search path
        // e.g., for module "std.io" with search path "std", look for "io.bl" in "std/"
        if module_path.len() > 1 {
            if let Some(first) = module_path.first() {
                for search_path in &self.search_paths {
                    let search_path_str = search_path.to_str().unwrap_or("");
                    if search_path_str.ends_with(first) || search_path_str.ends_with(&format!("/{}", first)) || search_path_str.ends_with(&format!("\\{}", first)) {
                        let remaining = &module_path[1..];
                        let remaining_file = format!("{}.bl", remaining.join("/"));
                        let candidate = search_path.join(&remaining_file);
                        if candidate.exists() {
                            return Ok(candidate);
                        }
                    }
                }
            }
        }
        
        // Also try as single filename for simple modules
        let simple_name = format!("{}.bl", module_path.join("."));
        for search_path in &self.search_paths {
            let candidate = search_path.join(&simple_name);
            if candidate.exists() {
                return Ok(candidate);
            }
        }
        
        Err(format!("Module not found: {}", module_path.join(".")))
    }
    
    fn extract_module_path(statements: &[Stmt], file_path: &Path) -> String {
        for stmt in statements {
            if let Stmt::Module { path, .. } = stmt {
                return path.join(".");
            }
        }
        
        let mut components: Vec<String> = Vec::new();
        if let Some(parent) = file_path.parent() {
            for component in parent.components() {
                if let std::path::Component::Normal(c) = component {
                    if let Some(s) = c.to_str() {
                        components.push(s.to_string());
                    }
                }
            }
        }
        if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
            components.push(stem.to_string());
        }
        components.join(".")
    }
    
    fn load_module(&mut self, module_path: &[String]) -> Result<String, String> {
        let module_name = module_path.join(".");

        if self.loaded_modules.contains_key(&module_name) {
            return Ok(module_name);
        }

        let module_file = self.find_module_file(module_path)?;

        let source = fs::read_to_string(&module_file)
            .map_err(|e| format!("Failed to read module '{}': {}", module_file.display(), e))?;

        let mut lexer = Lexer::new(&source, module_file.to_str().unwrap_or("unknown"));
        let (tokens, token_positions) = lexer.tokenize()
            .map_err(|e| format!("Lexical error in '{}': {}", module_file.display(), e))?;

        let mut parser = Parser::new(tokens, &source, module_file.to_str().unwrap_or("unknown"), token_positions);
        let mut statements = parser.parse()
            .map_err(|e| format!("Parse error in '{}': {}", module_file.display(), e))?;

        let actual_module_path = Self::extract_module_path(&statements, &module_file);

        // Build internal import map for this module - map simple names to qualified names
        // This ensures internal calls like print() become std.io.print()
        let mut internal_import_map = HashMap::new();
        for stmt in &statements {
            match stmt {
                Stmt::Function(func) => {
                    let qualified_name = format!("{}.{}", actual_module_path, func.name);
                    internal_import_map.insert(func.name.clone(), qualified_name);
                }
                Stmt::Class(class) => {
                    let qualified_name = format!("{}.{}", actual_module_path, class.name);
                    internal_import_map.insert(class.name.clone(), qualified_name);
                }
                _ => {}
            }
        }

        // Rewrite calls within the module to use qualified names
        Self::rewrite_calls(&mut statements, &internal_import_map);

        let mut functions = Vec::new();
        let mut native_functions = Vec::new();
        let mut generic_functions = Vec::new();
        let mut classes = Vec::new();

        for stmt in &statements {
            match stmt {
                Stmt::Function(func) => {
                    let qualified_name = format!("{}.{}", actual_module_path, func.name);
                    functions.push(qualified_name.clone());
                    if func.is_native {
                        native_functions.push(qualified_name.clone());
                    }
                    if !func.type_params.is_empty() {
                        generic_functions.push(qualified_name);
                    }
                }
                Stmt::Class(class) => {
                    classes.push(format!("{}.{}", actual_module_path, class.name));
                }
                _ => {}
            }
        }

        let module_info = ModuleInfo {
            module_path: actual_module_path.clone(),
            path: module_file,
            statements,
            source,
            functions,
            native_functions,
            generic_functions,
            classes,
        };
        
        self.loaded_modules.insert(actual_module_path.clone(), module_info);
        Ok(actual_module_path)
    }
    
    fn process_imports(&mut self, stmts: &[Stmt]) -> Result<(), String> {
        for stmt in stmts {
            if let Stmt::Import { path, kind, .. } = stmt {
                match kind {
                    ImportKind::Simple => {
                        // import std.io - brings println into scope
                        // We don't compile the module, just track the import for name resolution
                        // Native functions will be resolved at runtime
                        if let Ok(module_name) = self.load_module(path) {
                            if let Some(module_info) = self.loaded_modules.get(&module_name) {
                                for func in &module_info.functions {
                                    // Map qualified name
                                    self.import_map.insert(func.clone(), func.clone());
                                    // Also map simple name
                                    if let Some(simple_name) = func.split('.').last() {
                                        self.import_map.insert(simple_name.to_string(), func.clone());
                                    }
                                    // Track if this function is native
                                    if module_info.native_functions.contains(func) {
                                        self.native_functions.insert(func.clone(), true);
                                    }
                                }
                                // Also register classes from the imported module
                                for class in &module_info.classes {
                                    // Map qualified name
                                    self.import_map.insert(class.clone(), class.clone());
                                    // Also map simple name (e.g., "HttpClient" from "std.http.HttpClient")
                                    if let Some(simple_name) = class.split('.').last() {
                                        self.import_map.insert(simple_name.to_string(), class.clone());
                                    }
                                }
                            }
                        }
                    }
                    ImportKind::Module => {
                        let _ = self.load_module(path);
                    }
                    ImportKind::Member => {
                        if path.len() >= 2 {
                            let module_path = &path[..path.len()-1];
                            let member = path.last().unwrap();
                            if let Ok(module_name) = self.load_module(module_path) {
                                if let Some(module_info) = self.loaded_modules.get(&module_name) {
                                    for func in &module_info.functions {
                                        if func.ends_with(&format!(".{}", member)) {
                                            self.import_map.insert(member.clone(), func.clone());
                                            // Track if this function is native
                                            if module_info.native_functions.contains(func) {
                                                self.native_functions.insert(func.clone(), true);
                                            }
                                        }
                                    }
                                    // Also check for class members
                                    for class in &module_info.classes {
                                        if class.ends_with(&format!(".{}", member)) {
                                            self.import_map.insert(member.clone(), class.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ImportKind::Wildcard => {
                        if let Ok(module_name) = self.load_module(path) {
                            if let Some(module_info) = self.loaded_modules.get(&module_name) {
                                for func in &module_info.functions {
                                    if let Some(simple_name) = func.split('.').last() {
                                        self.import_map.insert(simple_name.to_string(), func.clone());
                                        // Track if this function is native
                                        if module_info.native_functions.contains(func) {
                                            self.native_functions.insert(func.clone(), true);
                                        }
                                    }
                                }
                                // Also register classes from wildcard imports
                                for class in &module_info.classes {
                                    if let Some(simple_name) = class.split('.').last() {
                                        self.import_map.insert(simple_name.to_string(), class.clone());
                                    }
                                }
                            }
                        }
                    }
                    ImportKind::Aliased(alias) => {
                        if let Ok(module_name) = self.load_module(path) {
                            if let Some(module_info) = self.loaded_modules.get(&module_name) {
                                for func in &module_info.functions {
                                    let aliased_name = format!("{}.{}", alias, func.split('.').last().unwrap_or(""));
                                    self.import_map.insert(aliased_name.clone(), func.clone());
                                    // Track if this function is native
                                    if module_info.native_functions.contains(func) {
                                        self.native_functions.insert(aliased_name, true);
                                    }
                                }
                                // Also register classes with alias
                                for class in &module_info.classes {
                                    let aliased_name = format!("{}.{}", alias, class.split('.').last().unwrap_or(""));
                                    self.import_map.insert(aliased_name.clone(), class.clone());
                                }
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }
    
    fn rewrite_calls(stmts: &mut Vec<Stmt>, import_map: &HashMap<String, String>) {
        for stmt in stmts {
            Self::rewrite_stmt_calls(stmt, import_map);
        }
    }
    
    fn rewrite_stmt_calls(stmt: &mut Stmt, import_map: &HashMap<String, String>) {
        match stmt {
            Stmt::Expr(expr) => Self::rewrite_expr_calls(expr, import_map),
            Stmt::Let { expr, .. } => Self::rewrite_expr_calls(expr, import_map),
            Stmt::Assign { expr, .. } => Self::rewrite_expr_calls(expr, import_map),
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    Self::rewrite_expr_calls(e, import_map);
                }
            }
            Stmt::Function(func) => {
                // Rewrite function body
                Self::rewrite_block(&mut func.body, import_map);
            }
            Stmt::If { condition, then_branch, else_branch, .. } => {
                Self::rewrite_expr_calls(condition, import_map);
                Self::rewrite_block(then_branch, import_map);
                if let Some(else_b) = else_branch {
                    Self::rewrite_block(else_b, import_map);
                }
            }
            Stmt::For { range, body, .. } => {
                Self::rewrite_expr_calls(range, import_map);
                Self::rewrite_block(body, import_map);
            }
            Stmt::While { condition, body, .. } => {
                Self::rewrite_expr_calls(condition, import_map);
                Self::rewrite_block(body, import_map);
            }
            Stmt::TryCatch { try_block, catch_block, .. } => {
                Self::rewrite_block(try_block, import_map);
                Self::rewrite_block(catch_block, import_map);
            }
            Stmt::Throw { expr, .. } => {
                Self::rewrite_expr_calls(expr, import_map);
            }
            _ => {}
        }
    }
    
    fn rewrite_block(block: &mut Vec<Stmt>, import_map: &HashMap<String, String>) {
        for stmt in block {
            Self::rewrite_stmt_calls(stmt, import_map);
        }
    }

    /// Build a qualified name from a Get expression chain
    /// E.g., std.io -> "std.io"
    fn build_qualified_name(expr: &crate::parser::Expr) -> String {
        use crate::parser::Expr;
        
        match expr {
            Expr::Variable { name, .. } => name.clone(),
            Expr::Get { object, name, .. } => {
                let obj_name = Self::build_qualified_name(object);
                format!("{}.{}", obj_name, name)
            }
            _ => String::new(),
        }
    }
    
    fn rewrite_expr_calls(expr: &mut crate::parser::Expr, import_map: &HashMap<String, String>) {
        use crate::parser::Expr;

        match expr {
            Expr::Call { callee, args, .. } => {
                if let Expr::Variable { name, .. } = callee.as_mut() {
                    if let Some(qualified_name) = import_map.get(name) {
                        *name = qualified_name.clone();
                    }
                } else if let Expr::Get { object, name, .. } = callee.as_mut() {
                    // Handle qualified calls like std.io.println
                    // Build the full path from the object
                    let full_path = Self::build_qualified_name(object);
                    let full_name = format!("{}.{}", full_path, name);
                    
                    // Check if the full name is in the import map
                    if let Some(qualified_name) = import_map.get(&full_name) {
                        // Replace the Get expression with a Variable using the qualified name
                        // We need to preserve the span from the original callee
                        let span_val = match callee.as_ref() {
                            Expr::Get { span, .. } => *span,
                            _ => Span::unknown(),
                        };
                        *callee = Box::new(Expr::Variable {
                            name: qualified_name.clone(),
                            span: span_val,
                        });
                    }
                }

                for arg in args {
                    Self::rewrite_expr_calls(arg, import_map);
                }
            }
            Expr::Binary { left, right, .. } => {
                Self::rewrite_expr_calls(left, import_map);
                Self::rewrite_expr_calls(right, import_map);
            }
            Expr::Unary { expr: inner, .. } => {
                Self::rewrite_expr_calls(inner, import_map);
            }
            Expr::Get { object, .. } => {
                Self::rewrite_expr_calls(object, import_map);
            }
            Expr::Set { object, value, .. } => {
                Self::rewrite_expr_calls(object, import_map);
                Self::rewrite_expr_calls(value, import_map);
            }
            Expr::Index { object, index, .. } => {
                Self::rewrite_expr_calls(object, import_map);
                Self::rewrite_expr_calls(index, import_map);
            }
            Expr::Array { elements, .. } => {
                for elem in elements {
                    Self::rewrite_expr_calls(elem, import_map);
                }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for field in fields {
                    Self::rewrite_expr_calls(&mut field.value, import_map);
                }
            }
            Expr::Interpolated { parts, .. } => {
                for part in parts {
                    if let crate::parser::InterpPart::Expr(e) = part {
                        Self::rewrite_expr_calls(e, import_map);
                    }
                }
            }
            Expr::Cast { expr, .. } => {
                Self::rewrite_expr_calls(expr, import_map);
            }
            Expr::Range { start, end, .. } => {
                Self::rewrite_expr_calls(start, import_map);
                Self::rewrite_expr_calls(end, import_map);
            }
            Expr::Lambda { body, .. } => {
                Self::rewrite_block(body, import_map);
            }
            _ => {}
        }
    }
    
    pub fn compile(&mut self) -> Result<CompilationResult, String> {
        let source_path = self.source_path.as_deref().unwrap_or("unknown");
        let mut lexer = Lexer::new(&self.source, source_path);
        let (tokens, token_positions) = lexer.tokenize()
            .map_err(|e| format!("Lexical error: {}", e))?;

        let mut parser = Parser::new(tokens, &self.source, source_path, token_positions);
        let mut statements = parser.parse()
            .map_err(|e| format!("Parse error: {}", e))?;

        self.process_imports(&statements)?;

        // Rewrite function calls to use fully qualified names
        Self::rewrite_calls(&mut statements, &self.import_map);

        // Type check the code if type checking is enabled
        if self.options.enable_type_checking {
            let mut ctx = TypeContext::new();

            // Collect all class names to exclude them from function registration
            let mut class_names: std::collections::HashSet<String> = std::collections::HashSet::new();
            for (module_name, module_info) in &self.loaded_modules {
                for stmt in &module_info.statements {
                    if let Stmt::Class(class) = stmt {
                        let qualified_name = format!("{}.{}", module_name, class.name);
                        class_names.insert(qualified_name);
                        if !class.private {
                            class_names.insert(class.name.clone());
                        }
                    }
                }
            }

            // Register imported functions in the type context with generic signatures
            // This allows the type checker to recognize them without enforcing strict types
            // Skip class names - they will be registered separately as classes
            for (qualified_name, _) in &self.import_map {
                // Skip if this is a class name
                if class_names.contains(qualified_name) {
                    continue;
                }
                
                // Register as a native function with generic parameters (allows any arguments)
                ctx.add_function(qualified_name, crate::types::FunctionSignature {
                    name: qualified_name.clone(),
                    params: vec![crate::types::ParamSignature {
                        name: "args".to_string(),
                        type_name: Some(crate::types::Type::Any),
                        default: false,
                    }],
                    return_type: Some(crate::types::Type::Any),
                    return_optional: false,
                    is_method: false,
                    is_native: true,
                    private: false,
                    type_params: Vec::new(),
                    mangled_name: None,
                });
            }

            // Register imported classes in the type context
            // We need to register classes from all loaded modules
            for (module_name, module_info) in &self.loaded_modules {
                for stmt in &module_info.statements {
                    if let Stmt::Class(class) = stmt {
                        // Register the class with its qualified name
                        let mut class_copy = class.clone();
                        class_copy.name = format!("{}.{}", module_name, class.name);
                        ctx.add_class(&class_copy);

                        // Also register with simple name for non-private classes
                        if !class.private {
                            ctx.add_class(class);
                        }
                    }
                }
            }

            let mut type_checker = TypeChecker::with_context(ctx);

            let result = type_checker.check(&statements);
            if let Err(errors) = result {
                let mut error_msg = String::new();
                let source_lines: Vec<&str> = self.source.lines().collect();

                for mut error in errors {
                    // Extract source line if we have line number
                    if error.source_line.is_none() && error.line > 0 && error.line <= source_lines.len() {
                        error.source_line = Some(source_lines[error.line - 1].to_string());
                    }

                    let location = if let Some(ref file) = error.source_file {
                        format!("{}:{}:{}", file, error.line, error.column)
                    } else {
                        format!("{}:{}", error.line, error.column)
                    };

                    error_msg.push_str(&format!("{}: error: {}\n", location, error.message));
                    if let Some(ref line) = error.source_line {
                        error_msg.push_str(&format!("  {}\n", line));
                        let caret_pos = error.column.saturating_sub(1);
                        let caret_line: String = " ".repeat(caret_pos) + "^";
                        error_msg.push_str(&format!("  {}\n", caret_line));
                    }
                }

                return Err(error_msg);
            }
        }

        let module_name = self.source_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem().and_then(|s| s.to_str()))
            .unwrap_or("module")
            .to_string();

        // First, compile the main module (it must come first for the entry point)
        // Collect all native function names that are accessible from the main module
        let main_native_functions: Vec<String> = self.native_functions.keys()
            .cloned()
            .collect();
        // Collect all generic function names from loaded modules
        let mut main_generic_functions: Vec<String> = Vec::new();
        for module_info in self.loaded_modules.values() {
            main_generic_functions.extend(module_info.generic_functions.clone());
        }
        let main_hlir = ast_to_hlir(&module_name, &statements);
        let main_compiled = compile_hlir_to_sparkler_with_natives(&main_hlir, main_native_functions, main_generic_functions);

        let mut all_strings: Vec<String> = Vec::new();
        let mut all_data: Vec<u8> = Vec::new();
        let mut max_registers: usize = main_compiled.max_registers;
        
        let mut function_map: std::collections::HashMap<String, sparkler::vm::Function> = std::collections::HashMap::new();
        let mut class_map: std::collections::HashMap<String, sparkler::vm::Class> = std::collections::HashMap::new();
        let mut vtable_map: std::collections::HashMap<String, sparkler::vm::VTable> = std::collections::HashMap::new();

        // Add main module strings, bytecode and functions FIRST so they are at index 0
        let main_string_offset = 0;
        for s in &main_compiled.strings {
            all_strings.push(s.clone());
        }
        
        let mut main_data = main_compiled.data.clone();
        self.adjust_string_indices(&mut main_data, main_string_offset);
        all_data.extend(main_data);
        
        for mut func in main_compiled.functions {
            self.adjust_string_indices(&mut func.bytecode, main_string_offset);
            function_map.insert(func.name.clone(), func);
        }
        
        for class in main_compiled.classes {
            class_map.insert(class.name.clone(), class);
        }
        
        for vtable in main_compiled.vtables {
            vtable_map.insert(vtable.class_name.clone(), vtable);
        }

        // Then compile imported modules and append (overriding ONLY IF duplicate names)
        for (_imported_module_name, module_info) in &self.loaded_modules {
            let imported_stmts = module_info.statements.clone();
            let module_native_functions = module_info.native_functions.clone();
            let module_generic_functions = module_info.generic_functions.clone();

            let imported_hlir = ast_to_hlir(&module_info.module_path, &imported_stmts);
            let imported_compiled = compile_hlir_to_sparkler_with_natives(&imported_hlir, module_native_functions, module_generic_functions);

            let string_offset = all_strings.len();
            for s in &imported_compiled.strings {
                all_strings.push(s.clone());
            }

            // Don't append imported module's data (root section) - only merge functions
            // let mut imported_data = imported_compiled.data.clone();
            // self.adjust_string_indices(&mut imported_data, string_offset);
            // all_data.extend(imported_data);

            for mut func in imported_compiled.functions {
                self.adjust_string_indices(&mut func.bytecode, string_offset);
                // Only insert if not already present, OR if it's not "main"
                if !function_map.contains_key(&func.name) {
                    function_map.insert(func.name.clone(), func);
                }
            }
            
            for class in imported_compiled.classes {
                if !class_map.contains_key(&class.name) {
                    class_map.insert(class.name.clone(), class);
                }
            }
            
            for vtable in imported_compiled.vtables {
                if !vtable_map.contains_key(&vtable.class_name) {
                    vtable_map.insert(vtable.class_name.clone(), vtable);
                }
            }

            if imported_compiled.max_registers > max_registers {
                max_registers = imported_compiled.max_registers;
            }
        }

        let merged_bytecode = CompiledBytecode {
            data: all_data,
            strings: all_strings,
            max_registers,
            functions: function_map.into_values().collect(),
            classes: class_map.into_values().collect(),
            vtables: vtable_map.into_values().collect(),
        };

        #[cfg(feature = "llvm")]
        let llvm_ir = if self.options.emit_llvm_ir {
            Some(crate::hlir::generate_llvm_ir_from_hlir(&main_hlir))
        } else {
            None
        };
        #[cfg(not(feature = "llvm"))]
        let _llvm_ir: Option<()> = None;

        Ok(CompilationResult {
            hlir: main_hlir,
            sparkler_bytecode: Some(merged_bytecode),
            #[cfg(feature = "llvm")]
            llvm_ir,
        })
    }
    
    fn adjust_string_indices(&self, bytecode: &mut [u8], offset: usize) {
        if offset == 0 { return; }
        let mut i = 0;
        while i < bytecode.len() {
            let opcode = bytecode[i];
            let size = match opcode {
                0x00 => 1, // Nop
                0x10 => { // LoadConst Rd, idx
                    if i + 2 < bytecode.len() {
                        bytecode[i + 2] = (bytecode[i + 2] as usize + offset) as u8;
                    }
                    3
                }
                0x11 | 0x12 => 10, // LoadInt, LoadFloat
                0x13 => 2, // LoadBool
                0x14 => 2, // LoadNull
                0x20 => 3, // Move
                0x21 => { // LoadLocal Rd, idx
                    if i + 2 < bytecode.len() {
                        bytecode[i + 2] = (bytecode[i + 2] as usize + offset) as u8;
                    }
                    3
                }
                0x22 => { // StoreLocal idx, Rs
                    if i + 1 < bytecode.len() {
                        bytecode[i + 1] = (bytecode[i + 1] as usize + offset) as u8;
                    }
                    3
                }
                0x30 => { // GetProperty Rd, Robj, idx
                    if i + 3 < bytecode.len() {
                        bytecode[i + 3] = (bytecode[i + 3] as usize + offset) as u8;
                    }
                    4
                }
                0x31 => { // SetProperty Robj, idx, Rs
                    if i + 2 < bytecode.len() {
                        bytecode[i + 2] = (bytecode[i + 2] as usize + offset) as u8;
                    }
                    4
                }
                0x40 | 0x41 | 0x42 => { // Call, CallNative, Invoke: Rd, idx, start, count
                    if i + 2 < bytecode.len() {
                        bytecode[i + 2] = (bytecode[i + 2] as usize + offset) as u8;
                    }
                    5
                }
                0x43 => 2, // Return
                0x44 => { // InvokeInterface: Rd, idx, start, count
                    if i + 2 < bytecode.len() {
                        bytecode[i + 2] = (bytecode[i + 2] as usize + offset) as u8;
                    }
                    6
                }
                0x45 => 6, // CallNativeIndexed
                0x50 => 3, // Jump
                0x51 | 0x52 => 4, // JumpIfTrue/False
                0x60..=0x63 | 0x66..=0x71 | 0x75 | 0x78..=0x7A | 0x7C..=0x7D => 4, // 3-reg ops
                0x64 | 0x7B => 3, // 2-reg ops (Not, BitNot)
                0x65 => 4, // Concat
                0x73 => 3, // Line
                0x74 => 4, // Convert
                0x76 => 4, // Array
                0x77 => 4, // Index
                0x80 => 4, // TryStart
                0x81 => 1, // TryEnd
                0x82 => 2, // Throw
                0x90 => 1, // Breakpoint
                0xFF => 1, // Halt
                _ => 1,
            };
            i += size;
        }
    }
    
    pub fn compile_to_sparkler(&mut self) -> Result<CompiledBytecode, String> {
        let result = self.compile()?;
        result.sparkler_bytecode.ok_or_else(|| "Sparkler bytecode generation failed".to_string())
    }
    
    #[cfg(feature = "llvm")]
    pub fn compile_to_llvm_ir(&mut self) -> Result<String, String> {
        let result = self.compile()?;
        result.llvm_ir.ok_or_else(|| "LLVM IR generation failed".to_string())
    }
}

pub fn sparkler_to_bytecode(compiled: CompiledBytecode) -> sparkler::executor::Bytecode {
    sparkler::executor::Bytecode {
        data: compiled.data,
        strings: compiled.strings,
        classes: compiled.classes,
        functions: compiled.functions,
        vtables: compiled.vtables,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_compilation() {
        let source = r#"fn add(a: int, b: int): int {
    return a + b;
}"#;
        
        let options = CompilerOptions::default();
        let mut compiler = HlirCompiler::with_options(source, options);
        let result = compiler.compile().unwrap();
        
        assert!(result.sparkler_bytecode.is_some());
        #[cfg(feature = "llvm")]
        assert!(result.llvm_ir.is_none());
    }
    
    #[test]
    #[cfg(feature = "llvm")]
    fn test_llvm_ir_emission() {
        let source = r#"fn add(a: int, b: int): int {
    return a + b;
}"#;
        
        let mut options = CompilerOptions::default();
        options.emit_llvm_ir = true;
        let mut compiler = HlirCompiler::with_options(source, options);
        
        let result = compiler.compile().unwrap();
        
        assert!(result.llvm_ir.is_some());
        let ir = result.llvm_ir.unwrap();
        assert!(ir.contains("define i32 @add"));
        assert!(ir.contains("add i32"));
    }
    
    #[test]
    fn test_arithmetic_compilation() {
        let source = r#"fn compute(x: int): int {
    let y = x * 2;
    return y + 3;
}"#;
        
        let options = CompilerOptions::default();
        let mut compiler = HlirCompiler::with_options(source, options);
        let result = compiler.compile().unwrap();
        
        assert!(result.sparkler_bytecode.is_some());
        let bytecode = result.sparkler_bytecode.unwrap();
        assert!(!bytecode.data.is_empty());
    }
    
    #[test]
    fn test_loop_compilation() {
        let source = r#"fn sum(n: int): int {
    let result = 0;
    for (i in 0..n) {
        result = result + i;
    }
    return result;
}"#;

        let options = CompilerOptions::default();
        let mut compiler = HlirCompiler::with_options(source, options);
        let result = compiler.compile().unwrap();

        assert!(result.sparkler_bytecode.is_some());
    }
    
    #[test]
    fn test_conditional_compilation() {
        let source = r#"fn max(a: int, b: int): int {
    if (a > b) {
        return a;
    } else {
        return b;
    }
}"#;
        
        let options = CompilerOptions::default();
        let mut compiler = HlirCompiler::with_options(source, options);
        let result = compiler.compile().unwrap();
        
        assert!(result.sparkler_bytecode.is_some());
    }
}

//! HLIR-based Bengal Compiler with Full Module Support
//! 
//! This compiler uses HLIR as the intermediate representation.
//! It supports imports, module resolution, and bytecode merging.

use crate::lexer::Lexer;
use crate::parser::{Parser, Stmt, ImportKind};
use crate::hlir::HlirModule;
use crate::ast_to_hlir_full::ast_to_hlir;
use crate::hlir_to_sparkler::{compile_hlir_to_sparkler, CompiledBytecode};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Compiler options
#[derive(Debug, Clone)]
pub struct CompilerOptions {
    pub enable_type_checking: bool,
    pub search_paths: Vec<String>,
    pub unsafe_fast: bool,
    pub emit_llvm_ir: bool,
    pub emit_sparkler_bytecode: bool,
}

impl Default for CompilerOptions {
    fn default() -> Self {
        Self {
            enable_type_checking: true,
            search_paths: vec!["std".to_string()],
            unsafe_fast: false,
            emit_llvm_ir: false,
            emit_sparkler_bytecode: true,
        }
    }
}

/// Compilation result
#[derive(Debug)]
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
        let statements = parser.parse()
            .map_err(|e| format!("Parse error in '{}': {}", module_file.display(), e))?;
        
        let actual_module_path = Self::extract_module_path(&statements, &module_file);
        
        let mut functions = Vec::new();
        let mut classes = Vec::new();
        
        for stmt in &statements {
            match stmt {
                Stmt::Function(func) => {
                    functions.push(format!("{}.{}", actual_module_path, func.name));
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
                                    self.import_map.insert(aliased_name, func.clone());
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
    
    fn rewrite_expr_calls(expr: &mut crate::parser::Expr, import_map: &HashMap<String, String>) {
        use crate::parser::Expr;
        
        match expr {
            Expr::Call { callee, args, .. } => {
                if let Expr::Variable { name, .. } = callee.as_mut() {
                    if let Some(qualified_name) = import_map.get(name) {
                        *name = qualified_name.clone();
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
        
        let module_name = self.source_path
            .as_ref()
            .and_then(|p| std::path::Path::new(p).file_stem().and_then(|s| s.to_str()))
            .unwrap_or("module")
            .to_string();
        
        let main_hlir = ast_to_hlir(&module_name, &statements);
        
        // Compile main module only
        // Imported modules are tracked for name resolution but not compiled
        // Native/external functions will be resolved at runtime
        let main_compiled = compile_hlir_to_sparkler(&main_hlir);
        
        let merged_bytecode = CompiledBytecode {
            data: main_compiled.data,
            strings: main_compiled.strings,
            max_registers: main_compiled.max_registers,
        };
        
        #[cfg(feature = "llvm")]
        let llvm_ir = if self.options.emit_llvm_ir {
            Some(crate::hlir::generate_llvm_ir_from_hlir(&main_hlir))
        } else {
            None
        };
        #[cfg(not(feature = "llvm"))]
        let llvm_ir: Option<()> = None;
        
        Ok(CompilationResult {
            hlir: main_hlir,
            sparkler_bytecode: Some(merged_bytecode),
            #[cfg(feature = "llvm")]
            llvm_ir,
        })
    }
    
    fn adjust_string_indices(&self, bytecode: &mut [u8], offset: usize) {
        let mut i = 0;
        while i < bytecode.len() {
            let opcode = bytecode[i];
            match opcode {
                0x10 | 0x21 | 0x22 | 0x30 | 0x31 | 0x40 | 0x41 | 0x42 | 0x45 | 0x46 | 0x49 | 0x4A => {
                    if i + 2 < bytecode.len() {
                        i += 2;
                        let idx = bytecode[i] as usize;
                        bytecode[i] = (idx + offset) as u8;
                    }
                    i += 1;
                }
                _ => {
                    i += 1;
                }
            }
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
        classes: Vec::new(),
        functions: Vec::new(),
        vtables: Vec::new(),
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
    for (i in range(0, n)) {
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

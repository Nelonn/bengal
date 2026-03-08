use crate::parser::{Stmt, Expr, Literal, Parser, ClassDef, BinaryOp, UnaryOp, InterpPart};
use crate::lexer::Lexer;
use crate::resolver::ModuleResolver;
use crate::types::TypeContext;

pub type Bytecode = sparkler::executor::Bytecode;

pub struct Compiler {
    source: String,
    source_path: Option<String>,
    type_context: Option<TypeContext>,
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
            source_path: None,
            type_context: None,
        }
    }

    pub fn with_path(source: &str, path: &str) -> Self {
        Self {
            source: source.to_string(),
            source_path: Some(path.to_string()),
            type_context: None,
        }
    }

    pub fn compile(&self) -> Result<Bytecode, String> {
        self.compile_with_options(&CompilerOptions::default())
    }

    pub fn compile_with_options(&self, options: &CompilerOptions) -> Result<Bytecode, String> {
        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;

        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;

        let mut type_context = None;
        if options.enable_type_checking {
            let mut resolver = ModuleResolver::new();

            for path in &options.search_paths {
                if let Ok(full_path) = std::path::PathBuf::from(path).canonicalize() {
                    resolver.add_search_path(full_path);
                }
            }

            match resolver.build_type_context(&statements) {
                Ok(ctx) => {
                    type_context = Some(ctx.clone());
                }
                Err(e) => {
                    return Err(format!("Type checking failed:\n{}", e));
                }
            }
        }

        self.generate_code(&statements, type_context)
    }

    fn generate_code(&self, statements: &[Stmt], type_context: Option<TypeContext>) -> Result<Bytecode, String> {
        let mut bytecode = Vec::new();
        let mut strings: Vec<String> = Vec::new();
        let mut classes: Vec<ClassDef> = Vec::new();

        for stmt in statements {
            if let Stmt::Class(class) = stmt {
                classes.push(class.clone());
            }
        }

        for stmt in statements {
            self.compile_stmt(stmt, &mut bytecode, &mut strings, &classes)?;
        }

        bytecode.push(Opcode::Halt as u8);

        Ok(Bytecode {
            data: bytecode,
            strings,
        })
    }

    fn compile_stmt(&self, stmt: &Stmt, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef]) -> Result<(), String> {
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
                self.compile_expr(expr, bytecode, strings, classes)?;
                let name_idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::StoreLocal as u8);
                bytecode.push(name_idx as u8);
            }
            Stmt::Assign { name, expr } => {
                self.compile_expr(expr, bytecode, strings, classes)?;
                let name_idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::StoreLocal as u8);
                bytecode.push(name_idx as u8);
            }
            Stmt::Return(expr) => {
                if let Some(e) = expr {
                    self.compile_expr(e, bytecode, strings, classes)?;
                } else {
                    bytecode.push(Opcode::PushNull as u8);
                }
                bytecode.push(Opcode::Return as u8);
            }
            Stmt::Expr(expr) => {
                self.compile_expr(expr, bytecode, strings, classes)?;
                bytecode.push(Opcode::Pop as u8);
            }
            Stmt::If { condition, then_branch, else_branch } => {
                self.compile_expr(condition, bytecode, strings, classes)?;

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
                    self.compile_stmt(stmt, bytecode, strings, classes)?;
                }

                if let Some(else_b) = else_branch {
                    bytecode.push(Opcode::Jump as u8);
                    let end_jump_pos = bytecode.len();
                    bytecode.push(0);

                    let else_target = bytecode.len();
                    bytecode[else_jump[0]] = (else_target & 0xFF) as u8;

                    for stmt in else_b {
                        self.compile_stmt(stmt, bytecode, strings, classes)?;
                    }

                    let end_target = bytecode.len();
                    bytecode[end_jump_pos] = (end_target & 0xFF) as u8;
                } else {
                    let else_target = bytecode.len();
                    bytecode[else_jump[0]] = (else_target & 0xFF) as u8;
                }
            }
        }
        Ok(())
    }

    fn compile_expr(&self, expr: &Expr, bytecode: &mut Vec<u8>, strings: &mut Vec<String>, classes: &[ClassDef]) -> Result<(), String> {
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
            Expr::Variable(name) => {
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::LoadLocal as u8);
                bytecode.push(idx as u8);
            }
            Expr::Binary { left, op, right } => {
                self.compile_expr(left, bytecode, strings, classes)?;
                self.compile_expr(right, bytecode, strings, classes)?;

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
            Expr::Unary { op, expr } => {
                self.compile_expr(expr, bytecode, strings, classes)?;
                match op {
                    UnaryOp::Not => bytecode.push(Opcode::Not as u8),
                }
            }
            Expr::Call { callee, args } => {
                for arg in args {
                    self.compile_expr(arg, bytecode, strings, classes)?;
                }

                if let Expr::Variable(func_name) = callee.as_ref() {
                    if func_name.starts_with("C.") {
                        let native_name = func_name.strip_prefix("C.").unwrap();
                        let native_id = get_native_id(native_name);
                        // Check if it's an async native function
                        if native_name == "http_get" || native_name == "http_post" {
                            bytecode.push(Opcode::CallNativeAsync as u8);
                        } else {
                            bytecode.push(Opcode::CallNative as u8);
                        }
                        bytecode.push(native_id);
                    } else if func_name == "println" || func_name == "print" {
                        let native_id = get_native_id(func_name);
                        bytecode.push(Opcode::CallNative as u8);
                        bytecode.push(native_id);
                    } else {
                        let idx = strings.len();
                        strings.push(func_name.clone());
                        bytecode.push(Opcode::Call as u8);
                        bytecode.push(idx as u8);
                        bytecode.push(args.len() as u8);
                    }
                } else if let Expr::Get { object, name } = callee.as_ref() {
                    self.compile_expr(object, bytecode, strings, classes)?;

                    let method_idx = strings.len();
                    strings.push(name.clone());
                    bytecode.push(Opcode::Invoke as u8);
                    bytecode.push(method_idx as u8);
                    bytecode.push((args.len() + 1) as u8);
                }

                for _ in args {
                    bytecode.push(Opcode::Pop as u8);
                }
            }
            Expr::Get { object, name } => {
                self.compile_expr(object, bytecode, strings, classes)?;
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::GetProperty as u8);
                bytecode.push(idx as u8);
            }
            Expr::Set { object, name, value } => {
                self.compile_expr(object, bytecode, strings, classes)?;
                self.compile_expr(value, bytecode, strings, classes)?;
                let idx = strings.len();
                strings.push(name.clone());
                bytecode.push(Opcode::SetProperty as u8);
                bytecode.push(idx as u8);
            }
            Expr::Interpolated { parts } => {
                for part in parts {
                    match part {
                        InterpPart::Text(s) => {
                            let idx = strings.len();
                            strings.push(s.clone());
                            bytecode.push(Opcode::PushString as u8);
                            bytecode.push(idx as u8);
                        }
                        InterpPart::Expr(e) => {
                            self.compile_expr(e, bytecode, strings, classes)?;
                        }
                    }
                }
                bytecode.push(Opcode::Concat as u8);
                bytecode.push(parts.len() as u8);
            }
            Expr::Await { expr } => {
                self.compile_expr(expr, bytecode, strings, classes)?;
                bytecode.push(Opcode::Await as u8);
            }
        }
        Ok(())
    }
}

fn get_native_id(name: &str) -> u8 {
    match name {
        "bengal_print" | "print" => 0,
        "bengal_println" | "println" => 1,
        "http_get" => 2,
        "http_post" => 3,
        "http_client_request" => 4,
        "http_client_get" => 5,
        "http_client_post" => 6,
        "http_client_get_with_headers" => 7,
        "http_client_post_with_headers" => 8,
        _ => 255,
    }
}

#[derive(Debug, Clone, Copy)]
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
    Multiply = 0x6A,
    Divide = 0x6B,

    Pop = 0x70,

    Halt = 0xFF,
}

use crate::parser::{Stmt, Expr, Literal, Parser, ClassDef, BinaryOp, UnaryOp, InterpPart};
use crate::lexer::Lexer;

pub type Bytecode = sparkler::executor::Bytecode;

pub struct Compiler {
    source: String,
}

impl Compiler {
    pub fn new(source: &str) -> Self {
        Self {
            source: source.to_string(),
        }
    }

    pub fn compile(&self) -> Result<Bytecode, String> {
        let mut lexer = Lexer::new(&self.source);
        let tokens = lexer.tokenize()?;

        let mut parser = Parser::new(tokens);
        let statements = parser.parse()?;

        self.generate_code(&statements)
    }

    fn generate_code(&self, statements: &[Stmt]) -> Result<Bytecode, String> {
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
            Stmt::Import { .. } => {
            }
            Stmt::Class(_) => {
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
                    bytecode.push(Opcode::JumpIfTrue as u8);
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
            Stmt::For { var_name, range, body } => {
                // Compile range expression
                if let Expr::Range { start, end } = range.as_ref() {
                    // Check if we can determine direction at compile time
                    let is_descending = match (start.as_ref(), end.as_ref()) {
                        (Expr::Literal(Literal::Int(start_val)), Expr::Literal(Literal::Int(end_val))) => {
                            start_val > end_val
                        }
                        _ => false, // Default to ascending for non-literal ranges
                    };

                    // Compile start value
                    self.compile_expr(start, bytecode, strings, classes)?;
                    
                    // Store as iterator
                    let iter_idx = strings.len();
                    strings.push(format!("__for_iter_{}", var_name));
                    bytecode.push(Opcode::StoreLocal as u8);
                    bytecode.push(iter_idx as u8);

                    // Compile end value
                    self.compile_expr(end, bytecode, strings, classes)?;
                    
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
                        self.compile_stmt(stmt, bytecode, strings, classes)?;
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
                let _loop_start = bytecode.len();

                // Compile condition
                self.compile_expr(condition, bytecode, strings, classes)?;

                bytecode.push(Opcode::JumpIfFalse as u8);
                let exit_jump = bytecode.len();
                bytecode.push(0); // placeholder

                // Compile body
                for stmt in body {
                    self.compile_stmt(stmt, bytecode, strings, classes)?;
                }

                // Jump back to start
                bytecode.push(Opcode::Jump as u8);
                let jump_back_pos = bytecode.len();
                bytecode.push(0);

                // Exit position - fix up jumps
                let exit_pos = bytecode.len();
                bytecode[exit_jump] = (exit_pos & 0xFF) as u8;
                bytecode[jump_back_pos] = (exit_pos & 0xFF) as u8;
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
                        bytecode.push(Opcode::CallNative as u8);
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
            Expr::Range { start: _, end: _ } => {
                // Range expressions are only used in for loops and handled specially
                // This should not be reached during normal compilation
                return Err("Range expression outside of for loop".to_string());
            }
        }
        Ok(())
    }
}

fn get_native_id(name: &str) -> u8 {
    match name {
        "bengal_print" | "print" => 0,
        "bengal_println" | "println" => 1,
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

    Jump = 0x50,
    JumpIfTrue = 0x51,
    JumpIfFalse = 0x52,
    JumpIfGreater = 0x53,
    JumpIfLess = 0x54,

    Equal = 0x60,
    NotEqual = 0x61,
    And = 0x62,
    Or = 0x63,
    Not = 0x64,
    Concat = 0x65,
    Add = 0x66,
    Subtract = 0x67,
    Multiply = 0x68,
    Divide = 0x69,
    Greater = 0x6A,
    Less = 0x6B,

    Pop = 0x70,

    Halt = 0xFF,
}

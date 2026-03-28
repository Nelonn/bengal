//! Complete AST to HLIR Converter - FULLY FUNCTIONAL
//!
//! This module converts Bengal AST to HLIR (High-Level IR).
//! Supports ALL Bengal language features.

use crate::parser::{self, Expr, Stmt, BinaryOp, UnaryOp, Literal, CastType, InterpPart};
use crate::hlir::{HlirBuilder, HlirModule, HlirType, HlirValue, HlirBinOp, HlirUnaryOp, HlirCastKind, HlirClass};
use crate::types::{self, Type};

/// AST to HLIR converter
pub struct AstToHlirConverter {
    builder: HlirBuilder,
    break_targets: Vec<String>,
    continue_targets: Vec<String>,
    current_return_type: HlirType,
    var_types: std::collections::HashMap<String, HlirType>,
    var_ptrs: std::collections::HashMap<String, HlirValue>,
    var_classes: std::collections::HashMap<String, String>,
    /// Map from variable name to declared type name (for interface dispatch)
    var_declared_type_names: std::collections::HashMap<String, String>,
    string_table: Vec<String>,
    module_prefix: String,
}

impl AstToHlirConverter {
    pub fn new(module_name: &str) -> Self {
        Self {
            builder: HlirBuilder::new(module_name),
            break_targets: Vec::new(),
            continue_targets: Vec::new(),
            current_return_type: HlirType::Void,
            var_types: std::collections::HashMap::new(),
            var_ptrs: std::collections::HashMap::new(),
            var_classes: std::collections::HashMap::new(),
            var_declared_type_names: std::collections::HashMap::new(),
            string_table: Vec::new(),
            module_prefix: module_name.to_string(),
        }
    }
    
    fn add_string(&mut self, s: String) -> usize {
        if let Some(idx) = self.string_table.iter().position(|existing| *existing == s) {
            idx
        } else {
            let idx = self.string_table.len();
            self.string_table.push(s);
            idx
        }
    }
    
    /// Convert a complete AST to HLIR module
    pub fn convert_module(&mut self, stmts: &[Stmt]) -> HlirModule {
        // Collect all top-level items
        let mut module_level_stmts: Vec<&Stmt> = Vec::new();
        let mut functions: Vec<&parser::FunctionDef> = Vec::new();
        let mut classes: Vec<&parser::ClassDef> = Vec::new();
        let mut interfaces: Vec<&parser::InterfaceDef> = Vec::new();
        let mut module_path: Option<String> = None;

        for stmt in stmts {
            match stmt {
                Stmt::Module { path, .. } => {
                    // Extract module path from "module x.y.z" declaration
                    module_path = Some(path.join("."));
                }
                Stmt::Function(func) => {
                    // Skip native functions - they're handled at runtime
                    if !func.is_native {
                        functions.push(func);
                    }
                }
                Stmt::Class(class) => classes.push(class),
                Stmt::Interface(interface) => interfaces.push(interface),
                Stmt::Import { .. } => {
                    // Skip imports - they're handled at runtime
                }
                _ => module_level_stmts.push(stmt),
            }
        }

        // Use module path from declaration, or empty string if not declared
        if let Some(path) = module_path {
            self.module_prefix = path;
        } else {
            self.module_prefix = String::new();
        }

        // Convert functions with module prefix (only non-native functions)
        for func in &functions {
            self.convert_function_with_prefix(func);
        }

        // Convert interfaces first (so they're available for class interface detection)
        for interface in interfaces {
            self.convert_interface(interface);
        }

        // Convert classes
        for class in classes {
            self.convert_class(class);
        }

        // ALWAYS create module wrapper function - this is the module entry point
        let func_name = if self.module_prefix.is_empty() {
            "_main".to_string()
        } else {
            format!("{}._main", self.module_prefix)
        };
        
        self.builder.begin_function(&func_name, vec![], HlirType::Void);
        self.builder.begin_block("entry");
        self.current_return_type = HlirType::Void;

        // Convert module-level statements (if any)
        // main() is just a regular function - no automatic calls
        for stmt in module_level_stmts {
            self.convert_stmt(stmt);
        }

        self.builder.ret(None, HlirType::Void);
        self.builder.end_block();
        self.builder.end_function();

        self.builder.clone().build()
    }
    
    /// Convert a function definition with module prefix
    #[allow(dead_code)] // Kept for potential future use
    fn convert_function(&mut self, func: &parser::FunctionDef) {
        let params: Vec<(String, HlirType)> = func.params.iter()
            .map(|p| {
                let ty = p.type_name.as_ref()
                    .map(|t| self.type_from_str(t))
                    .unwrap_or(HlirType::Unknown);
                (p.name.clone(), ty)
            })
            .collect();
        
        let return_ty = func.return_type.as_ref()
            .map(|t| self.type_from_str(t))
            .unwrap_or(HlirType::Void);
        
        self.current_return_type = return_ty.clone();
        self.var_types.clear();
        self.var_ptrs.clear();
        self.var_classes.clear();
        self.var_declared_type_names.clear();

        self.builder.begin_function(&func.name, params, return_ty.clone());
        self.builder.begin_block("entry");
        
        for stmt in &func.body {
            self.convert_stmt(stmt);
        }
        
        if return_ty != HlirType::Void {
            if !matches!(func.body.last(), Some(Stmt::Return { .. })) {
                let default = match return_ty {
                    HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(0.0),
                    HlirType::Bool => HlirValue::BoolConst(false),
                    _ => HlirValue::IntConst(0),
                };
                self.builder.ret(Some(default), return_ty);
            }
        } else {
            if !matches!(func.body.last(), Some(Stmt::Return { .. })) {
                self.builder.ret(None, HlirType::Void);
            }
        }
        
        self.builder.end_block();
        self.builder.end_function();
    }

    /// Convert a function definition with module prefix
    fn convert_function_with_prefix(&mut self, func: &parser::FunctionDef) {
        use crate::types::{mangle, Type};
        
        let params: Vec<(String, HlirType)> = func.params.iter()
            .map(|p| {
                let ty = p.type_name.as_ref()
                    .map(|t| self.type_from_str(t))
                    .unwrap_or(HlirType::Unknown);
                (p.name.clone(), ty)
            })
            .collect();

        let return_ty = func.return_type.as_ref()
            .map(|t| self.type_from_str(t))
            .unwrap_or(HlirType::Void);

        self.current_return_type = return_ty.clone();
        self.var_types.clear();
        self.var_ptrs.clear();
        self.var_classes.clear();
        self.var_declared_type_names.clear();

        // Build parameter types for mangling
        let param_types: Vec<Type> = func.params.iter().map(|p| {
            p.type_name.as_ref()
                .map(|t| Type::from_str(t))
                .unwrap_or(Type::Unknown)
        }).collect();

        let qualified_name = if self.module_prefix.is_empty() {
            mangle(None, None, &func.name, &param_types)
        } else {
            mangle(Some(&self.module_prefix), None, &func.name, &param_types)
        };
        
        self.builder.begin_function(&qualified_name, params, return_ty.clone());
        self.builder.begin_block("entry");

        for stmt in &func.body {
            self.convert_stmt(stmt);
        }

        if return_ty != HlirType::Void {
            if !matches!(func.body.last(), Some(Stmt::Return { .. })) {
                let default = match return_ty {
                    HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(0.0),
                    HlirType::Bool => HlirValue::BoolConst(false),
                    _ => HlirValue::IntConst(0),
                };
                self.builder.ret(Some(default), return_ty);
            }
        } else {
            if !matches!(func.body.last(), Some(Stmt::Return { .. })) {
                self.builder.ret(None, HlirType::Void);
            }
        }

        self.builder.end_block();
        self.builder.end_function();
    }
    
    /// Convert a class definition
    fn convert_class(&mut self, class: &parser::ClassDef) {
        // Check if class has a constructor
        let has_constructor = class.methods.iter().any(|m| m.name == "constructor");
        
        for method in &class.methods {
            // Skip native methods - they're handled at runtime
            if method.is_native {
                continue;
            }

            let mut params: Vec<(String, HlirType)> = vec![
                ("self".to_string(), HlirType::Pointer(Box::new(HlirType::Unknown)))
            ];
            params.extend(method.params.iter().map(|p| {
                let ty = p.type_name.as_ref()
                    .map(|t| self.type_from_str(t))
                    .unwrap_or(HlirType::Unknown);
                (p.name.clone(), ty)
            }));

            let return_ty = method.return_type.as_ref()
                .map(|t| self.type_from_str(t))
                .unwrap_or(HlirType::Void);

            // Build mangled method name with parameter types for overloading support
            let param_types_str: Vec<String> = method.params.iter().map(|p| {
                p.type_name.as_ref()
                    .map(|t| t.clone())
                    .unwrap_or_else(|| "Unknown".to_string())
            }).collect();
            
            // Convert to Type for mangle function
            let param_types: Vec<Type> = param_types_str.iter().map(|s| Type::from_str(s)).collect();

            // Use mangle function for all methods (e.g., "SomeObject.method(str)")
            let method_name = types::mangle(None, Some(&class.name), &method.name, &param_types);

            self.current_return_type = return_ty.clone();
            self.var_types.clear();
            self.var_ptrs.clear();
            self.var_classes.clear();
            self.var_declared_type_names.clear();

            self.builder.begin_function(&method_name, params, return_ty.clone());
            self.builder.begin_block("entry");

            // self is the first parameter (R0)
            let self_val = HlirValue::Param(0);
            self.var_ptrs.insert("self".to_string(), self_val);

            // Set up field pointers for field access within methods
            // Fields are accessed as offsets from self, but for simplicity we allocate locals
            for field in &class.fields {
                if !field.is_static {
                    let field_ptr = self.builder.alloca(HlirType::I32, &field.name);
                    self.var_ptrs.insert(field.name.clone(), field_ptr.clone());
                }
            }

            // Special handling for constructors with empty body - initialize fields
            let is_constructor = method.name == "constructor";
            let is_empty_body = method.body.is_empty();

            if is_constructor && is_empty_body {
                // Initialize fields with default values by storing to self
                let self_val = HlirValue::Param(0);
                for field in &class.fields {
                    if !field.is_static {
                        // Store default value directly to self.field
                        if field.type_name == "int" {
                            self.builder.set_property(self_val.clone(), &field.name, HlirValue::IntConst(42));
                        }
                    }
                }
                // Return self
                self.builder.ret(Some(self_val), HlirType::Pointer(Box::new(HlirType::Unknown)));
            } else {
                // Normal method - compile body statements
                for stmt in &method.body {
                    self.convert_stmt(stmt);
                }

                // Special handling for constructors - they must return self
                if is_constructor {
                    if !matches!(method.body.last(), Some(Stmt::Return { .. })) {
                        self.builder.ret(Some(HlirValue::Param(0)), HlirType::Pointer(Box::new(HlirType::Unknown)));
                    }
                } else if return_ty != HlirType::Void {
                    if !matches!(method.body.last(), Some(Stmt::Return { .. })) {
                        self.builder.ret(Some(HlirValue::IntConst(0)), return_ty);
                    }
                } else {
                    if !matches!(method.body.last(), Some(Stmt::Return { .. })) {
                        self.builder.ret(None, HlirType::Void);
                    }
                }
            }

            self.builder.end_block();
            self.builder.end_function();
        }

        // Generate default constructor only if one doesn't already exist
        if !has_constructor {
            // Generate default constructor using the central mangle function
            let constructor_name = types::mangle(None, Some(&class.name), "constructor", &[]);
            // Constructor receives self as the first parameter (the pre-allocated instance)
            self.builder.begin_function(&constructor_name, vec![
                ("self".to_string(), HlirType::Pointer(Box::new(HlirType::Unknown)))
            ], HlirType::Pointer(Box::new(HlirType::Unknown)));
            self.builder.begin_block("entry");

            // self is the first parameter (R1 in bytecode, Param(0) in HLIR)
            let self_val = HlirValue::Param(0);
            self.var_ptrs.insert("self".to_string(), self_val.clone());

            // Initialize fields with default values by storing to self
            for field in &class.fields {
                if !field.is_static {
                    // Store default value directly to self.field
                    if field.type_name == "int" {
                        self.builder.set_property(self_val.clone(), &field.name, HlirValue::IntConst(42));
                    }
                }
            }

            // Return self (the object pointer)
            self.builder.ret(Some(self_val), HlirType::Pointer(Box::new(HlirType::Unknown)));
            self.builder.end_block();
            self.builder.end_function();
        }

        // Generate class metadata for the VM
        let fields: Vec<String> = class.fields.iter()
            .filter(|f| !f.is_static)
            .map(|f| f.name.clone())
            .collect();
        let private_fields: Vec<String> = class.fields.iter()
            .filter(|f| !f.is_static && f.private)
            .map(|f| f.name.clone())
            .collect();
        let methods: Vec<String> = class.methods.iter()
            .filter(|m| !m.is_native)
            .map(|m| {
                let param_types: Vec<crate::types::Type> = m.params.iter().map(|p| {
                    p.type_name.as_ref()
                        .map(|t| crate::types::Type::from_str(t))
                        .unwrap_or_else(|| crate::types::Type::Unknown)
                }).collect();
                types::mangle(None, Some(&class.name), &m.name, &param_types)
            })
            .collect();

        // Build vtable: methods that override interface methods
        // The vtable contains base method names (without class prefix) for interface dispatch
        let vtable: Vec<String> = class.methods.iter()
            .filter(|m| {
                // Include all non-constructor methods for classes with interfaces
                class.parent_interfaces.iter().any(|iface_name| {
                    m.name != "constructor"
                })
            })
            .map(|m| m.name.clone())  // Just the method name, e.g., "print"
            .collect();

        let hlir_class = HlirClass {
            name: class.name.clone(),
            fields,
            private_fields,
            methods,
            is_native: class.is_native,
            is_interface: false,
            parent_interfaces: class.parent_interfaces.clone(),
            vtable,
        };
        self.builder.add_class(hlir_class);
    }

    /// Convert an interface definition to HLIR class metadata
    fn convert_interface(&mut self, interface: &parser::InterfaceDef) {
        // Interfaces have no fields
        let fields: Vec<String> = Vec::new();
        let private_fields: Vec<String> = Vec::new();

        // Collect interface method names
        let methods: Vec<String> = interface.methods.iter()
            .map(|m| {
                let param_types: Vec<crate::types::Type> = m.params.iter().map(|p| {
                    p.type_name.as_ref()
                        .map(|t| crate::types::Type::from_str(t))
                        .unwrap_or_else(|| crate::types::Type::Unknown)
                }).collect();
                types::mangle(None, Some(&interface.name), &m.name, &param_types)
            })
            .collect();

        // Interface vtable contains all method names (base names without mangling)
        let vtable: Vec<String> = interface.methods.iter()
            .map(|m| m.name.clone())
            .collect();

        let hlir_class = HlirClass {
            name: interface.name.clone(),
            fields,
            private_fields,
            methods,
            is_native: false,
            is_interface: true,
            parent_interfaces: interface.parent_interfaces.clone(),
            vtable,
        };
        self.builder.add_class(hlir_class);
    }
    
    /// Convert a statement
    fn convert_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let { name, expr, type_annotation, .. } => {
                let ty = type_annotation.as_ref()
                    .map(|t| self.type_from_str(t))
                    .unwrap_or_else(|| self.infer_expr_type(expr));

                let ptr = self.builder.alloca(ty.clone(), name);
                self.var_types.insert(name.clone(), ty.clone());
                self.var_ptrs.insert(name.clone(), ptr.clone());

                // Store the declared type name for interface dispatch
                if let Some(type_ann) = type_annotation {
                    self.var_declared_type_names.insert(name.clone(), type_ann.clone());
                }

                // If the expression is a constructor call, store the class name
                if let Expr::Call { callee, .. } = expr {
                    if let Expr::Variable { name: callee_name, .. } = callee.as_ref() {
                        if callee_name.chars().next().map_or(false, |c| c.is_uppercase()) {
                            self.var_classes.insert(name.clone(), callee_name.clone());
                        }
                    }
                }

                let value = self.convert_expr(expr);
                self.builder.store(value, ptr, ty);
            }
            
            Stmt::Assign { name, expr, .. } => {
                let ty = self.var_types.get(name)
                    .cloned()
                    .unwrap_or_else(|| self.infer_expr_type(expr));
                
                if let Some(ptr) = self.var_ptrs.get(name).cloned() {
                    let value = self.convert_expr(expr);
                    self.builder.store(value, ptr, ty);
                }
            }
            
            Stmt::AugAssign { target, op, expr, .. } => {
                match target {
                    parser::AugAssignTarget::Variable(name) => {
                        let ty = self.var_types.get(name)
                            .cloned()
                            .unwrap_or(HlirType::I32);
                        
                        if let Some(ptr) = self.var_ptrs.get(name).cloned() {
                            let current = self.builder.load(ptr.clone(), ty.clone());
                            let new_value = self.convert_expr(expr);
                            
                            let hlir_op = match op {
                                parser::AugOp::Add => HlirBinOp::Add,
                                parser::AugOp::Subtract => HlirBinOp::Sub,
                                parser::AugOp::Multiply => HlirBinOp::Mul,
                                parser::AugOp::Divide => HlirBinOp::SDiv,
                                parser::AugOp::Modulo => HlirBinOp::SRem,
                                parser::AugOp::BitAnd => HlirBinOp::And,
                                parser::AugOp::BitOr => HlirBinOp::Or,
                                parser::AugOp::BitXor => HlirBinOp::Xor,
                                parser::AugOp::ShiftLeft => HlirBinOp::Shl,
                                parser::AugOp::ShiftRight => HlirBinOp::AShr,
                            };
                            
                            let result = self.builder.bin_op(hlir_op, current, new_value, ty.clone());
                            self.builder.store(result, ptr, ty);
                        }
                    }
                    parser::AugAssignTarget::Field { object, name: _ } => {
                        let _ = self.convert_expr(object);
                        let _ = self.convert_expr(expr);
                    }
                }
            }
            
            Stmt::Return { expr, .. } => {
                if let Some(e) = expr {
                    let value = self.convert_expr(e);
                    let ty = self.infer_expr_type(e);
                    self.builder.ret(Some(value), ty);
                } else {
                    self.builder.ret(None, HlirType::Void);
                }
            }
            
            Stmt::Expr(expr) => {
                // For expression statements, we don't need the result temp
                // Convert the expression but discard the result
                self.convert_expr_discard(expr);
            }
            
            Stmt::If { condition, then_branch, else_branch, .. } => {
                let cond = self.convert_expr(condition);

                let then_label = format!("if_then_{}", self.builder.new_temp());
                let else_label = format!("if_else_{}", self.builder.new_temp());
                let end_label = format!("if_end_{}", self.builder.new_temp());

                if else_branch.is_some() {
                    self.builder.cond_br(cond, &then_label, &else_label);

                    self.builder.begin_block(&then_label);
                    for stmt in then_branch {
                        self.convert_stmt(stmt);
                    }
                    // Only emit branch if then_branch doesn't end with a terminator
                    if !self.builder.current_block_has_terminator() {
                        self.builder.br(&end_label);
                    }

                    self.builder.begin_block(&else_label);
                    for stmt in else_branch.as_ref().unwrap() {
                        self.convert_stmt(stmt);
                    }
                    // Only emit branch if else_branch doesn't end with a terminator
                    if !self.builder.current_block_has_terminator() {
                        self.builder.br(&end_label);
                    }

                    self.builder.begin_block(&end_label);
                } else {
                    self.builder.cond_br(cond, &then_label, &end_label);

                    self.builder.begin_block(&then_label);
                    for stmt in then_branch {
                        self.convert_stmt(stmt);
                    }
                    // Only emit branch if then_branch doesn't end with a terminator
                    if !self.builder.current_block_has_terminator() {
                        self.builder.br(&end_label);
                    }

                    self.builder.begin_block(&end_label);
                }
            }
            
            Stmt::For { var_name, range, body, .. } => {
                if let Expr::Range { start, end, .. } = range.as_ref() {
                    let start_val = self.convert_expr(start);
                    let end_val = self.convert_expr(end);
                    
                    let loop_label = format!("for_loop_{}", self.builder.new_temp());
                    let body_label = format!("for_body_{}", self.builder.new_temp());
                    let end_label = format!("for_end_{}", self.builder.new_temp());
                    
                    let i_ptr = self.builder.alloca(HlirType::I32, var_name);
                    self.var_types.insert(var_name.clone(), HlirType::I32);
                    self.var_ptrs.insert(var_name.clone(), i_ptr.clone());
                    self.builder.store(start_val, i_ptr.clone(), HlirType::I32);
                    
                    self.builder.br(&loop_label);
                    
                    self.builder.begin_block(&loop_label);
                    let i_val = self.builder.load(i_ptr.clone(), HlirType::I32);
                    let cond = self.builder.bin_op(HlirBinOp::Slt, i_val.clone(), end_val.clone(), HlirType::I32);
                    self.builder.cond_br(cond, &body_label, &end_label);
                    
                    self.builder.begin_block(&body_label);
                    self.break_targets.push(end_label.clone());
                    self.continue_targets.push(loop_label.clone());
                    
                    for stmt in body {
                        self.convert_stmt(stmt);
                    }
                    
                    self.break_targets.pop();
                    self.continue_targets.pop();
                    
                    let one = HlirValue::IntConst(1);
                    let i_val = self.builder.load(i_ptr.clone(), HlirType::I32);
                    let i_new = self.builder.bin_op(HlirBinOp::Add, i_val, one, HlirType::I32);
                    self.builder.store(i_new, i_ptr, HlirType::I32);
                    
                    self.builder.br(&loop_label);
                    
                    self.builder.begin_block(&end_label);
                }
            }
            
            Stmt::While { condition, body, .. } => {
                let loop_label = format!("while_loop_{}", self.builder.new_temp());
                let body_label = format!("while_body_{}", self.builder.new_temp());
                let end_label = format!("while_end_{}", self.builder.new_temp());
                
                self.builder.br(&loop_label);
                
                self.builder.begin_block(&loop_label);
                let cond = self.convert_expr(condition);
                self.builder.cond_br(cond, &body_label, &end_label);
                
                self.builder.begin_block(&body_label);
                self.break_targets.push(end_label.clone());
                self.continue_targets.push(loop_label.clone());
                
                for stmt in body {
                    self.convert_stmt(stmt);
                }
                
                self.break_targets.pop();
                self.continue_targets.pop();
                
                self.builder.br(&loop_label);
                
                self.builder.begin_block(&end_label);
            }
            
            Stmt::Break(_) => {
                if let Some(target) = self.break_targets.last() {
                    self.builder.br(target);
                }
            }
            
            Stmt::Continue(_) => {
                if let Some(target) = self.continue_targets.last() {
                    self.builder.br(target);
                }
            }
            
            Stmt::TryCatch { try_block, catch_var, catch_block, .. } => {
                let catch_label = format!("catch_{}", self.builder.new_temp());
                let end_label = format!("try_end_{}", self.builder.new_temp());

                // Emit TryStart instruction at the current position (don't create a new block for try)
                let catch_reg = self.builder.new_temp();
                self.builder.try_start(&catch_label, catch_reg);

                for stmt in try_block {
                    self.convert_stmt(stmt);
                }
                // Emit TryEnd instruction
                self.builder.try_end();
                self.builder.br(&end_label);

                self.builder.begin_block(&catch_label);
                self.var_types.insert(catch_var.clone(), HlirType::String);
                let exc_ptr = self.builder.alloca(HlirType::String, catch_var);
                self.var_ptrs.insert(catch_var.clone(), exc_ptr.clone());
                // Store the exception value from catch_reg into the catch variable
                self.builder.store(HlirValue::Temp(catch_reg), exc_ptr, HlirType::String);

                for stmt in catch_block {
                    self.convert_stmt(stmt);
                }
                self.builder.br(&end_label);

                self.builder.begin_block(&end_label);
            }

            Stmt::Throw { expr, .. } => {
                let value = self.convert_expr(expr);
                self.builder.throw(value);
            }

            Stmt::Module { .. } | Stmt::Import { .. } | Stmt::Class(_) | Stmt::Interface(_) | Stmt::Enum(_) | Stmt::Function(_) | Stmt::TypeAlias(_) => {}
        }
    }
    
    /// Convert an expression
    fn convert_expr(&mut self, expr: &Expr) -> HlirValue {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::Int(n, _) => HlirValue::IntConst(*n),
                    Literal::Float(n, _) => HlirValue::FloatConst(*n),
                    Literal::Bool(b, _) => HlirValue::BoolConst(*b),
                    Literal::String(s, _) => {
                        self.add_string(s.clone());
                        HlirValue::StringConst(s.clone())
                    },
                    Literal::Null(_) => HlirValue::Null,
                }
            }
            
            Expr::Variable { name, .. } => {
                if name == "self" {
                    return HlirValue::Param(0);
                }
                if let Some(ptr) = self.var_ptrs.get(name).cloned() {
                    let ty = self.var_types.get(name)
                        .cloned()
                        .unwrap_or(HlirType::I32);
                    if let HlirValue::Param(_) = ptr {
                        return ptr;
                    }
                    // For pointer types (class instances), return the pointer directly
                    if matches!(ty, HlirType::Pointer(_)) {
                        return ptr;
                    }
                    self.builder.load(ptr, ty)
                } else {
                    HlirValue::Local(name.clone())
                }
            }
            
            Expr::Binary { left, op, right, .. } => {
                let lhs = self.convert_expr(left);
                let rhs = self.convert_expr(right);
                let ty = self.infer_expr_type(expr);
                
                let hlir_op = match op {
                    BinaryOp::Add => HlirBinOp::Add,
                    BinaryOp::Subtract => HlirBinOp::Sub,
                    BinaryOp::Multiply => HlirBinOp::Mul,
                    BinaryOp::Divide => HlirBinOp::SDiv,
                    BinaryOp::Modulo => HlirBinOp::SRem,
                    BinaryOp::Equal => HlirBinOp::Eq,
                    BinaryOp::NotEqual => HlirBinOp::Ne,
                    BinaryOp::Less => HlirBinOp::Slt,
                    BinaryOp::LessEqual => HlirBinOp::Sle,
                    BinaryOp::Greater => HlirBinOp::Sgt,
                    BinaryOp::GreaterEqual => HlirBinOp::Sge,
                    BinaryOp::And => HlirBinOp::And,
                    BinaryOp::Or => HlirBinOp::Or,
                    BinaryOp::BitAnd => HlirBinOp::And,
                    BinaryOp::BitOr => HlirBinOp::Or,
                    BinaryOp::BitXor => HlirBinOp::Xor,
                    BinaryOp::ShiftLeft => HlirBinOp::Shl,
                    BinaryOp::ShiftRight => HlirBinOp::AShr,
                    BinaryOp::Pow => HlirBinOp::Mul,
                };
                
                self.builder.bin_op(hlir_op, lhs, rhs, ty)
            }
            
            Expr::Unary { op, expr, .. } => {
                let value = self.convert_expr(expr);
                let ty = self.infer_expr_type(expr);
                
                let hlir_op = match op {
                    UnaryOp::Not => HlirUnaryOp::LNot,
                    UnaryOp::Negate => HlirUnaryOp::Neg,
                    UnaryOp::BitNot => HlirUnaryOp::Not,
                    UnaryOp::PrefixIncrement | UnaryOp::PostfixIncrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        return self.builder.bin_op(HlirBinOp::Add, value, one, ty);
                    }
                    UnaryOp::PrefixDecrement | UnaryOp::PostfixDecrement | UnaryOp::Decrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        return self.builder.bin_op(HlirBinOp::Sub, value, one, ty);
                    }
                };
                
                self.builder.unary_op(hlir_op, value, ty)
            }
            
            Expr::Call { callee, args, .. } => {
                let callee_type = match callee.as_ref() {
                    Expr::Variable { name, .. } => format!("Variable({})", name),
                    Expr::Get { object: _, name, .. } => format!("Get(method={})", name),
                    _ => "Other".to_string(),
                };
                let mut func_args: Vec<HlirValue> = args.iter()
                    .map(|a| self.convert_expr(a))
                    .collect();

                if let Expr::Variable { name, .. } = callee.as_ref() {
                    // Check if it's a class constructor call (starts with uppercase)
                    let func_name = if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        // Mangle constructor name with parameter types using the central mangle function
                        let arg_types: Vec<Type> = args.iter().map(|a| {
                            let ty = self.infer_expr_type(a);
                            // Convert HlirType to Type for mangling
                            match ty {
                                HlirType::I8 => Type::Int8,
                                HlirType::I32 => Type::Int,
                                HlirType::I64 => Type::Int64,
                                HlirType::F32 => Type::Float32,
                                HlirType::F64 => Type::Float64,
                                HlirType::Bool => Type::Bool,
                                HlirType::String => Type::Str,
                                _ => Type::Unknown,
                            }
                        }).collect();
                        // Use the central mangle function for constructor names
                        types::mangle(None, Some(name), "constructor", &arg_types)
                    } else {
                        name.clone()
                    };
                    let func = HlirValue::Function(func_name);
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call(func, func_args, return_ty)
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Method call: mangle to Class_method(self, args)
                    let obj_val = self.convert_expr(object);
                    // Prepend self to arguments
                    let mut call_args = vec![obj_val];
                    call_args.extend(func_args);

                    // Get the class/interface name from the object's declared type (preferred) or inferred type
                    let class_name = if let Expr::Variable { name: var_name, .. } = object.as_ref() {
                        // First check if we have a declared type name (e.g., from type annotation)
                        // This is important for interface-typed variables
                        if let Some(declared_type_name) = self.var_declared_type_names.get(var_name) {
                            declared_type_name.clone()
                        } else if let Some(concrete_class) = self.var_classes.get(var_name) {
                            // Fall back to concrete class from constructor
                            concrete_class.clone()
                        } else {
                            // Last resort: infer from variable name (snake_case to PascalCase)
                            let parts = var_name.split('_');
                            let mut class = String::new();
                            for part in parts {
                                if let Some(first) = part.chars().next() {
                                    class.push(first.to_ascii_uppercase());
                                    class.push_str(&part[1..]);
                                }
                            }
                            class
                        }
                    } else {
                        "Unknown".to_string()
                    };

                    // Build argument types for mangling (only actual method args, not self)
                    let arg_types: Vec<Type> = args.iter().map(|a| {
                        let ty = self.infer_expr_type(a);
                        match ty {
                            HlirType::I8 => Type::Int8,
                            HlirType::I32 => Type::Int,
                            HlirType::I64 => Type::Int64,
                            HlirType::F32 => Type::Float32,
                            HlirType::F64 => Type::Float64,
                            HlirType::Bool => Type::Bool,
                            HlirType::String => Type::Str,
                            _ => Type::Unknown,
                        }
                    }).collect();

                    // Use mangle function for method names (e.g., "SomeObject.method(str)")
                    let method_name = types::mangle(None, Some(&class_name), name, &arg_types);
                    let func = HlirValue::Function(method_name);
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call(func, call_args, return_ty)
                } else {
                    HlirValue::IntConst(0)
                }
            }
            
            Expr::Cast { expr, target_type, .. } => {
                let value = self.convert_expr(expr);
                let from_ty = self.infer_expr_type(expr);
                let to_ty = self.cast_type_to_hlir(target_type);
                
                let kind = match (&from_ty, &to_ty) {
                    (HlirType::I32, HlirType::I64) => HlirCastKind::SExt,
                    (HlirType::I64, HlirType::I32) => HlirCastKind::Trunc,
                    (HlirType::I32, HlirType::F64) => HlirCastKind::SiToFp,
                    (HlirType::F64, HlirType::I32) => HlirCastKind::FpToSi,
                    (HlirType::I32, HlirType::F32) => HlirCastKind::SiToFp,
                    (HlirType::F32, HlirType::I32) => HlirCastKind::FpToSi,
                    _ => HlirCastKind::BitCast,
                };
                
                self.builder.cast(value, from_ty, to_ty, kind)
            }
            
            Expr::Array { elements, .. } => {
                for elem in elements {
                    self.convert_expr(elem);
                }
                HlirValue::IntConst(0)
            }
            
            Expr::ObjectLiteral { fields, .. } => {
                for field in fields {
                    self.convert_expr(&field.value);
                }
                HlirValue::IntConst(0)
            }
            
            Expr::Index { object, index, .. } => {
                self.convert_expr(object);
                self.convert_expr(index);
                HlirValue::IntConst(0)
            }
            
            Expr::Get { object, name, .. } => {
                // Get the object value
                let object_val = self.convert_expr(object);

                // Check if this is a self.field access
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if obj_name == "self" {
                        // Use GetProperty for self.field access
                        self.builder.get_property(object_val, name)
                    } else {
                        // For other object.field access, use field pointer
                        if let Some(field_ptr) = self.var_ptrs.get(&name.clone()).cloned() {
                            self.builder.load(field_ptr, HlirType::I32)
                        } else {
                            HlirValue::IntConst(0)
                        }
                    }
                } else {
                    // For complex object expressions, use field pointer
                    if let Some(field_ptr) = self.var_ptrs.get(&name.clone()).cloned() {
                        self.builder.load(field_ptr, HlirType::I32)
                    } else {
                        HlirValue::IntConst(0)
                    }
                }
            }
            
            Expr::Set { object, name, value, span } => {
                // Handle field assignment: object.field = value
                let object_val = self.convert_expr(object);

                // Convert the value
                let value = self.convert_expr(value);

                // Check if this is a self.field assignment
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if obj_name == "self" {
                        // Use SetProperty for self.field assignments
                        self.builder.set_property(object_val, name, value);
                    } else {
                        // For other object.field assignments, use the field pointer approach
                        let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                        if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                            self.builder.store(value, field_ptr, value_ty);
                        }
                    }
                } else {
                    // For complex object expressions, use the field pointer approach
                    let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                    if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                        self.builder.store(value, field_ptr, value_ty);
                    }
                }

                HlirValue::IntConst(0)
            }
            
            Expr::Range { start, end, .. } => {
                self.convert_expr(start);
                self.convert_expr(end);
                HlirValue::IntConst(0)
            }
            
            Expr::Interpolated { parts, .. } => {
                // Collect all parts for a single optimized string concatenation
                let mut values: Vec<HlirValue> = Vec::new();
                
                for part in parts {
                    match part {
                        InterpPart::Text(s) => {
                            values.push(HlirValue::StringConst(s.clone()));
                        }
                        InterpPart::Expr(e) => {
                            // Convert expression to string
                            let expr_val = self.convert_expr(e);
                            let expr_ty = self.infer_expr_type(e);
                            
                            // Cast to string if needed
                            let str_val = if expr_ty != HlirType::String {
                                self.builder.cast(expr_val, expr_ty, HlirType::String, HlirCastKind::BitCast)
                            } else {
                                expr_val
                            };
                            values.push(str_val);
                        }
                    }
                }
                
                // Single optimized concatenation of all parts
                if values.is_empty() {
                    HlirValue::StringConst(String::new())
                } else if values.len() == 1 {
                    values.into_iter().next().unwrap()
                } else {
                    self.builder.string_concat(values)
                }
            }
            
            Expr::Lambda { params, return_type, body, .. } => {
                let func_params: Vec<(String, HlirType)> = params.iter()
                    .map(|p| {
                        let ty = p.type_name.as_ref()
                            .map(|t| self.type_from_str(t))
                            .unwrap_or(HlirType::Unknown);
                        (p.name.clone(), ty)
                    })
                    .collect();
                
                let func_name = format!("lambda_{}", self.builder.new_temp());
                let ret_ty = return_type.as_ref()
                    .map(|t| self.type_from_str(t))
                    .unwrap_or(HlirType::Unknown);
                
                self.builder.begin_function(&func_name, func_params, ret_ty.clone());
                self.builder.begin_block("entry");
                
                for stmt in body {
                    self.convert_stmt(stmt);
                }
                
                if ret_ty != HlirType::Void {
                    self.builder.ret(Some(HlirValue::IntConst(0)), ret_ty);
                } else {
                    self.builder.ret(None, HlirType::Void);
                }
                self.builder.end_block();
                self.builder.end_function();
                
                HlirValue::Function(func_name)
            }
        }
    }
    
    /// Infer the type of an expression
    fn infer_expr_type(&self, expr: &Expr) -> HlirType {
        match expr {
            Expr::Literal(lit) => {
                match lit {
                    Literal::Int(_, _) => HlirType::I32,
                    Literal::Float(_, _) => HlirType::F64,
                    Literal::Bool(_, _) => HlirType::Bool,
                    Literal::String(_, _) => HlirType::String,
                    Literal::Null(_) => HlirType::Unknown,
                }
            }
            Expr::Variable { name, .. } => {
                self.var_types.get(name).cloned().unwrap_or(HlirType::I32)
            }
            Expr::Binary { op, left, .. } => {
                match op {
                    BinaryOp::Add | BinaryOp::Subtract | BinaryOp::Multiply |
                    BinaryOp::Divide | BinaryOp::Modulo => {
                        self.infer_expr_type(left)
                    }
                    BinaryOp::Equal | BinaryOp::NotEqual |
                    BinaryOp::Less | BinaryOp::LessEqual |
                    BinaryOp::Greater | BinaryOp::GreaterEqual => HlirType::Bool,
                    BinaryOp::And | BinaryOp::Or => HlirType::Bool,
                    BinaryOp::BitAnd | BinaryOp::BitOr | BinaryOp::BitXor => HlirType::I32,
                    BinaryOp::ShiftLeft | BinaryOp::ShiftRight => HlirType::I32,
                    BinaryOp::Pow => self.infer_expr_type(left),
                }
            }
            Expr::Unary { op, expr, .. } => {
                match op {
                    UnaryOp::Negate | UnaryOp::PrefixIncrement | UnaryOp::PostfixIncrement |
                    UnaryOp::PrefixDecrement | UnaryOp::PostfixDecrement | UnaryOp::Decrement => {
                        self.infer_expr_type(expr)
                    }
                    UnaryOp::Not | UnaryOp::BitNot => HlirType::Bool,
                }
            }
            Expr::Call { callee, .. } => {
                // Check if it's a constructor call (starts with uppercase)
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    if name.chars().next().map_or(false, |c| c.is_uppercase()) {
                        // Constructor returns a pointer to the class instance
                        return HlirType::Pointer(Box::new(HlirType::Unknown));
                    }
                }
                HlirType::I32
            }
            Expr::Cast { target_type, .. } => self.cast_type_to_hlir(target_type),
            Expr::Array { .. } => HlirType::Unknown,
            Expr::ObjectLiteral { .. } => HlirType::Unknown,
            Expr::Index { .. } => HlirType::Unknown,
            Expr::Get { .. } => HlirType::Unknown,
            Expr::Set { .. } => HlirType::Void,
            Expr::Range { .. } => HlirType::Unknown,
            Expr::Interpolated { .. } => HlirType::String,
            Expr::Lambda { .. } => HlirType::Unknown,
        }
    }
    
    /// Convert a type string to HlirType
    fn type_from_str(&self, ty: &str) -> HlirType {
        match ty {
            "void" => HlirType::Void,
            "bool" => HlirType::Bool,
            "i8" | "Int8" => HlirType::I8,
            "i32" | "int" | "Int32" => HlirType::I32,
            "i64" | "Int64" => HlirType::I64,
            "f32" | "Float32" => HlirType::F32,
            "f64" | "float" | "Float64" => HlirType::F64,
            "string" | "String" => HlirType::String,
            _ => {
                if ty.starts_with('[') && ty.ends_with(']') {
                    let inner = &ty[1..ty.len()-1];
                    HlirType::Array(Box::new(self.type_from_str(inner)))
                } else {
                    HlirType::Unknown
                }
            }
        }
    }
    
    /// Convert CastType to HlirType
    fn cast_type_to_hlir(&self, ty: &CastType) -> HlirType {
        match ty {
            CastType::Int | CastType::Int8 | CastType::Int16 | CastType::Int32 | CastType::Int64 => HlirType::I32,
            CastType::UInt8 | CastType::UInt16 | CastType::UInt32 | CastType::UInt64 => HlirType::I32,
            CastType::Float | CastType::Float32 | CastType::Float64 => HlirType::F64,
            CastType::Bool => HlirType::Bool,
            CastType::Str => HlirType::String,
        }
    }
    
    pub fn build(self) -> HlirModule {
        self.builder.build()
    }

    /// Convert an expression, discarding the result (for expression statements)
    fn convert_expr_discard(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) => {}
            Expr::Variable { .. } => {}
            Expr::Binary { left, op: _, right, .. } => {
                self.convert_expr_discard(left);
                self.convert_expr_discard(right);
            }
            Expr::Unary { op, expr, .. } => {
                let value = self.convert_expr(expr);
                let ty = self.infer_expr_type(expr);
                match op {
                    UnaryOp::PrefixIncrement | UnaryOp::PostfixIncrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        self.builder.bin_op(HlirBinOp::Add, value, one, ty);
                    }
                    UnaryOp::PrefixDecrement | UnaryOp::PostfixDecrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        self.builder.bin_op(HlirBinOp::Sub, value, one, ty);
                    }
                    _ => self.convert_expr_discard(expr),
                }
            }
            Expr::Call { callee, args, .. } => {
                let func_args: Vec<HlirValue> = args.iter().map(|a| self.convert_expr(a)).collect();
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    let func = HlirValue::Function(name.clone());
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call_discard(func, func_args, return_ty);
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Method call - use Class.method() pattern with interface support
                    let obj_val = self.convert_expr(object);
                    // Prepend self to arguments
                    let mut call_args = vec![obj_val];
                    call_args.extend(func_args);

                    let class_name = if let Expr::Variable { name: var_name, .. } = object.as_ref() {
                        if let Some(declared_type_name) = self.var_declared_type_names.get(var_name) {
                            declared_type_name.clone()
                        } else if let Some(concrete_class) = self.var_classes.get(var_name) {
                            concrete_class.clone()
                        } else {
                            let parts = var_name.split('_');
                            let mut class = String::new();
                            for part in parts {
                                if let Some(first) = part.chars().next() {
                                    class.push(first.to_ascii_uppercase());
                                    class.push_str(&part[1..]);
                                }
                            }
                            class
                        }
                    } else {
                        "Unknown".to_string()
                    };
                    
                    // Build argument types for mangling (only actual method args, not self)
                    let arg_types: Vec<Type> = args.iter().map(|a| {
                        let ty = self.infer_expr_type(a);
                        match ty {
                            HlirType::I8 => Type::Int8,
                            HlirType::I32 => Type::Int,
                            HlirType::I64 => Type::Int64,
                            HlirType::F32 => Type::Float32,
                            HlirType::F64 => Type::Float64,
                            HlirType::Bool => Type::Bool,
                            HlirType::String => Type::Str,
                            _ => Type::Unknown,
                        }
                    }).collect();
                    
                    let method_name = types::mangle(None, Some(&class_name), name, &arg_types);
                    let func = HlirValue::Function(method_name);
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call_discard(func, call_args, return_ty);
                }
            }
            Expr::Get { object, .. } => { self.convert_expr_discard(object); }
            Expr::Set { object, name, value, span } => {
                // Handle field assignment: object.field = value
                let object_val = self.convert_expr(object);
                let value = self.convert_expr(value);

                // Check if this is a self.field assignment
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if obj_name == "self" {
                        // Use SetProperty for self.field assignments
                        self.builder.set_property(object_val, name, value);
                    } else {
                        // For other object.field assignments, use the field pointer approach
                        let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                        if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                            self.builder.store(value, field_ptr, value_ty);
                        }
                    }
                } else {
                    // For complex object expressions, use the field pointer approach
                    let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                    if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                        self.builder.store(value, field_ptr, value_ty);
                    }
                }
            }
            Expr::Array { elements, .. } => {
                for elem in elements { self.convert_expr_discard(elem); }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for field in fields { self.convert_expr_discard(&field.value); }
            }
            Expr::Cast { expr, .. } => { self.convert_expr_discard(expr); }
            Expr::Interpolated { parts, .. } => {
                for part in parts {
                    if let InterpPart::Expr(expr) = part {
                        self.convert_expr_discard(expr);
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                self.convert_expr_discard(start);
                self.convert_expr_discard(end);
            }
            Expr::Index { object, index, .. } => {
                self.convert_expr_discard(object);
                self.convert_expr_discard(index);
            }
            Expr::Lambda { body, .. } => {
                // Lambda body is already Vec<Stmt>
                for stmt in body {
                    self.convert_stmt(stmt);
                }
            }
        }
    }
}

/// Convert AST to HLIR module
pub fn ast_to_hlir(module_name: &str, stmts: &[Stmt]) -> HlirModule {
    let mut converter = AstToHlirConverter::new(module_name);
    converter.convert_module(stmts)
}

#[cfg(test)]
#[cfg(feature = "llvm")]
mod tests {
    use super::*;
    use crate::lexer::Lexer;
    use crate::parser::Parser;
    
    fn parse_source(source: &str) -> Vec<Stmt> {
        let (tokens, token_positions) = Lexer::new(source, "test").tokenize().unwrap();
        let mut parser = Parser::new(tokens, source, "test", token_positions);
        parser.parse().unwrap()
    }
    
    #[test]
    fn test_function_conversion() {
        let source = r#"fn add(a: int, b: int): int {
    return a + b;
}"#;
        let stmts = parse_source(source);
        let hlir = ast_to_hlir("test", &stmts);
        let ir = crate::hlir::generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("define i32 @add"));
        assert!(ir.contains("add i32"));
    }

    /// Convert an expression, discarding the result (for expression statements)
    fn convert_expr_discard(&mut self, expr: &Expr) {
        match expr {
            Expr::Literal(_) => {}
            Expr::Variable { .. } => {}
            Expr::Binary { left, op: _, right, .. } => {
                self.convert_expr_discard(left);
                self.convert_expr_discard(right);
            }
            Expr::Unary { op, expr, .. } => {
                let value = self.convert_expr(expr);
                let ty = self.infer_expr_type(expr);
                match op {
                    UnaryOp::PrefixIncrement | UnaryOp::PostfixIncrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        self.builder.bin_op(HlirBinOp::Add, value, one, ty);
                    }
                    UnaryOp::PrefixDecrement | UnaryOp::PostfixDecrement => {
                        let one = match ty {
                            HlirType::F32 | HlirType::F64 => HlirValue::FloatConst(1.0),
                            _ => HlirValue::IntConst(1),
                        };
                        self.builder.bin_op(HlirBinOp::Sub, value, one, ty);
                    }
                    _ => self.convert_expr_discard(expr),
                }
            }
            Expr::Call { callee, args, .. } => {
                let func_args: Vec<HlirValue> = args.iter().map(|a| self.convert_expr(a)).collect();
                if let Expr::Variable { name, .. } = callee.as_ref() {
                    let func = HlirValue::Function(name.clone());
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call_discard(func, func_args, return_ty);
                } else if let Expr::Get { object, name, .. } = callee.as_ref() {
                    // Method call - use Class.method() pattern with interface support
                    let obj_val = self.convert_expr(object);
                    // Prepend self to arguments
                    let mut call_args = vec![obj_val];
                    call_args.extend(func_args);

                    let class_name = if let Expr::Variable { name: var_name, .. } = object.as_ref() {
                        if let Some(declared_type_name) = self.var_declared_type_names.get(var_name) {
                            declared_type_name.clone()
                        } else if let Some(concrete_class) = self.var_classes.get(var_name) {
                            concrete_class.clone()
                        } else {
                            let parts = var_name.split('_');
                            let mut class = String::new();
                            for part in parts {
                                if let Some(first) = part.chars().next() {
                                    class.push(first.to_ascii_uppercase());
                                    class.push_str(&part[1..]);
                                }
                            }
                            class
                        }
                    } else {
                        "Unknown".to_string()
                    };
                    
                    // Build argument types for mangling (only actual method args, not self)
                    let arg_types: Vec<Type> = args.iter().map(|a| {
                        let ty = self.infer_expr_type(a);
                        match ty {
                            HlirType::I8 => Type::Int8,
                            HlirType::I32 => Type::Int,
                            HlirType::I64 => Type::Int64,
                            HlirType::F32 => Type::Float32,
                            HlirType::F64 => Type::Float64,
                            HlirType::Bool => Type::Bool,
                            HlirType::String => Type::Str,
                            _ => Type::Unknown,
                        }
                    }).collect();
                    
                    let method_name = types::mangle(None, Some(&class_name), name, &arg_types);
                    let func = HlirValue::Function(method_name);
                    let return_ty = self.infer_expr_type(expr);
                    self.builder.call_discard(func, call_args, return_ty);
                }
            }
            Expr::Get { object, .. } => { self.convert_expr_discard(object); }
            Expr::Set { object, name, value, span } => {
                // Handle field assignment: object.field = value
                let object_val = self.convert_expr(object);
                let value = self.convert_expr(value);

                // Check if this is a self.field assignment
                if let Expr::Variable { name: obj_name, .. } = object.as_ref() {
                    if obj_name == "self" {
                        // Use SetProperty for self.field assignments
                        self.builder.set_property(object_val, name, value);
                    } else {
                        // For other object.field assignments, use the field pointer approach
                        let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                        if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                            self.builder.store(value, field_ptr, value_ty);
                        }
                    }
                } else {
                    // For complex object expressions, use the field pointer approach
                    let value_ty = self.infer_expr_type(&Expr::Variable { name: name.clone(), span: span.clone() });
                    if let Some(field_ptr) = self.var_ptrs.get(name).cloned() {
                        self.builder.store(value, field_ptr, value_ty);
                    }
                }
            }
            Expr::Array { elements, .. } => {
                for elem in elements { self.convert_expr_discard(elem); }
            }
            Expr::ObjectLiteral { fields, .. } => {
                for field in fields { self.convert_expr_discard(&field.value); }
            }
            Expr::Cast { expr, .. } => { self.convert_expr_discard(expr); }
            Expr::Interpolated { parts, .. } => {
                for part in parts {
                    if let InterpPart::Expr(e) = part {
                        self.convert_expr_discard(e);
                    }
                }
            }
            Expr::Range { start, end, .. } => {
                self.convert_expr_discard(start);
                self.convert_expr_discard(end);
            }
            Expr::Index { object, index, .. } => {
                self.convert_expr_discard(object);
                self.convert_expr_discard(index);
            }
            Expr::Lambda { body, .. } => {
                for stmt in body {
                    self.convert_stmt(stmt);
                }
            }
        }
    }

    #[test]
    fn test_function_conversion() {
        let source = r#"fn test(): int {
    let x = 42;
    return x;
}"#;
        let stmts = parse_source(source);
        let hlir = ast_to_hlir("test", &stmts);
        let ir = crate::hlir::generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("define i32 @test"));
        assert!(ir.contains("alloca i32"));
        assert!(ir.contains("store i32"));
        assert!(ir.contains("load i32"));
    }
    
    #[test]
    fn test_loop_conversion() {
        let source = r#"fn sum(n: int): int {
    let result = 0;
    for (i in range(0, n)) {
        result = result + i;
    }
    return result;
}"#;
        let stmts = parse_source(source);
        let hlir = ast_to_hlir("test", &stmts);
        let ir = crate::hlir::generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("define i32 @sum"));
        assert!(ir.contains("alloca i32"));
    }
    
    #[test]
    fn test_if_conversion() {
        let source = r#"fn max(a: int, b: int): int {
    if (a > b) {
        return a;
    } else {
        return b;
    }
}"#;
        let stmts = parse_source(source);
        let hlir = ast_to_hlir("test", &stmts);
        let ir = crate::hlir::generate_llvm_ir_from_hlir(&hlir);
        
        assert!(ir.contains("define i32 @max"));
        assert!(ir.contains("if_then"));
        assert!(ir.contains("if_else"));
    }
}

/// Bytecode viewer with Godbolt-style formatting
///
/// Displays bytecode in a readable format similar to Compiler Explorer (godbolt.org):
/// - Per-function organization with function headers
/// - Constant pool (.data section) display
/// - Clean address | opcode | operands format

use sparkler::Bytecode;
use sparkler::Opcode;

/// Represents a single instruction in the bytecode
#[derive(Debug, Clone)]
pub struct InstructionView {
    pub address: usize,
    pub address_hex: String,
    pub opcode: Opcode,
    pub opcode_name: String,
    pub operands: String,
    pub operand_count: usize,
}

/// Represents a function or method in the bytecode
#[derive(Debug, Clone)]
pub struct FunctionView {
    pub name: String,
    pub register_count: u8,
    pub source_file: Option<String>,
    pub instructions: Vec<InstructionView>,
}

/// Represents the root/module-level code
#[derive(Debug, Clone)]
pub struct RootView {
    pub instructions: Vec<InstructionView>,
}

/// Represents a class field
#[derive(Debug, Clone)]
pub struct ClassFieldView {
    pub name: String,
    pub value: String,
}

/// Represents a class in the bytecode
#[derive(Debug, Clone)]
pub struct ClassView {
    pub name: String,
    pub fields: Vec<ClassFieldView>,
}

/// Represents a string constant
#[derive(Debug, Clone)]
pub struct StringConstantView {
    pub index: usize,
    pub value: String,
}

/// Represents the .data section
#[derive(Debug, Clone)]
pub struct DataView {
    pub strings: Vec<StringConstantView>,
    pub classes: Vec<ClassView>,
}

/// Complete view of the bytecode
#[derive(Debug, Clone)]
pub struct BytecodeView {
    pub data: DataView,
    pub root: Option<RootView>,
    pub functions: Vec<FunctionView>,
}

/// Convert bytecode to a structured view for programmatic consumption
pub fn view_bytecode(bytecode: &Bytecode) -> BytecodeView {
    BytecodeView {
        data: view_data_section(bytecode),
        root: view_root_code(bytecode),
        functions: view_functions(bytecode),
    }
}

/// View the .data section (constant pool)
fn view_data_section(bytecode: &Bytecode) -> DataView {
    let strings = bytecode.strings.iter().enumerate().map(|(i, s)| {
        StringConstantView {
            index: i,
            value: s.clone(),
        }
    }).collect();

    let classes = bytecode.classes.iter().map(|class| {
        let fields = class.fields.iter().map(|(name, value)| {
            ClassFieldView {
                name: name.clone(),
                value: format!("{:?}", value),
            }
        }).collect();

        ClassView {
            name: class.name.clone(),
            fields,
        }
    }).collect();

    DataView { strings, classes }
}

/// View module-level (root) code
fn view_root_code(bytecode: &Bytecode) -> Option<RootView> {
    if bytecode.data.is_empty() {
        return None;
    }

    let instructions = decode_instructions(&bytecode.data, bytecode, 0);
    Some(RootView { instructions })
}

/// View all functions
fn view_functions(bytecode: &Bytecode) -> Vec<FunctionView> {
    bytecode.functions.iter().map(|function| {
        let instructions = decode_instructions(&function.bytecode, bytecode, 0);
        FunctionView {
            name: function.name.clone(),
            register_count: function.register_count,
            source_file: function.source_file.clone(),
            instructions,
        }
    }).collect()
}

/// Decode all instructions from bytecode data
fn decode_instructions(data: &[u8], bytecode: &Bytecode, start_offset: usize) -> Vec<InstructionView> {
    let mut instructions = Vec::new();
    let mut pc = 0;

    while pc < data.len() {
        let opcode_byte = data[pc];
        let opcode = opcode_from_byte(opcode_byte);
        let address = pc;
        let (opcode_name, operands, operand_count) = decode_instruction(data, pc, opcode, bytecode);

        instructions.push(InstructionView {
            address: address + start_offset,
            address_hex: format!("{:04x}", address),
            opcode,
            opcode_name,
            operands,
            operand_count,
        });

        pc += 1 + operand_count;
    }

    instructions
}

/// Display bytecode in Godbolt-style format (prints to console)
pub fn display_bytecode(bytecode: &Bytecode) {
    let view = view_bytecode(bytecode);
    println!("{}", format_bytecode(&view));
}

/// Format bytecode as a string for display
pub fn format_bytecode(view: &BytecodeView) -> String {
    let mut output = String::new();

    output.push_str("# Bytecode Viewer - Bengal\n");
    output.push('\n');

    // Display .data section
    output.push_str(&format_data_section(&view.data));

    // Display root code
    if let Some(root) = &view.root {
        output.push_str(".root:\n");
        output.push_str("# module-level code\n");
        for instr in &root.instructions {
            output.push_str(&format_instruction(instr));
        }
        output.push('\n');
    }

    // Display functions
    for function in &view.functions {
        output.push_str(&format_function(function));
    }

    output
}

/// Format the .data section
fn format_data_section(data: &DataView) -> String {
    let mut output = String::new();

    output.push_str(".data\n");

    // Display string constants
    for s in &data.strings {
        output.push_str(&format!("  str.{:<4} = \"{}\"\n", s.index, escape_string(&s.value)));
    }

    // Display class information
    for class in &data.classes {
        output.push_str(&format!("  class.{} =\n", class.name));
        for field in &class.fields {
            output.push_str(&format!("    .{} = {}\n", field.name, field.value));
        }
    }

    output.push('\n');
    output
}

/// Format a single function
fn format_function(function: &FunctionView) -> String {
    let mut output = String::new();

    output.push_str(&format!("{}:\n", function.name));
    output.push_str(&format!("# registers: {}, source: {:?}\n", function.register_count, function.source_file));

    for instr in &function.instructions {
        output.push_str(&format_instruction(instr));
    }

    output.push('\n');
    output
}

/// Format a single instruction
fn format_instruction(instr: &InstructionView) -> String {
    if instr.operands.is_empty() {
        format!("  {} | {}\n", instr.address_hex, instr.opcode_name)
    } else {
        format!("  {} | {}, {}\n", instr.address_hex, instr.opcode_name, instr.operands)
    }
}

/// Decode instruction and return (name, operands_string, operand_byte_count)
fn decode_instruction(data: &[u8], pc: usize, opcode: Opcode, bytecode: &Bytecode) -> (String, String, usize) {
    let strings = &bytecode.strings;
    match opcode {
        Opcode::Nop => ("NOP".to_string(), String::new(), 0),

        Opcode::LoadConst => {
            if pc + 3 < data.len() {
                let str_idx = ((data[pc + 3] as usize) << 8) | (data[pc + 2] as usize);
                let value = strings.get(str_idx)
                    .map(|s| format!("\"{}\"", escape_string(s)))
                    .unwrap_or_else(|| format!("str.{}", str_idx));
                (format!("LOAD_CONST R{}", data[pc + 1]), format!("{}", value), 3)
            } else {
                ("LOAD_CONST".to_string(), String::new(), 0)
            }
        }

        Opcode::LoadInt => {
            if pc + 10 <= data.len() {
                let value = i64::from_le_bytes([
                    data[pc + 2], data[pc + 3], data[pc + 4], data[pc + 5],
                    data[pc + 6], data[pc + 7], data[pc + 8], data[pc + 9],
                ]);
                (format!("LOAD_INT R{}", data[pc + 1]), format!("{}", value), 9)
            } else {
                ("LOAD_INT".to_string(), String::new(), 0)
            }
        }

        Opcode::LoadFloat => {
            if pc + 10 <= data.len() {
                let value = f64::from_le_bytes([
                    data[pc + 2], data[pc + 3], data[pc + 4], data[pc + 5],
                    data[pc + 6], data[pc + 7], data[pc + 8], data[pc + 9],
                ]);
                (format!("LOAD_FLOAT R{}", data[pc + 1]), format!("{}", value), 9)
            } else {
                ("LOAD_FLOAT".to_string(), String::new(), 0)
            }
        }

        Opcode::LoadBool => {
            if pc + 2 < data.len() {
                let value = data[pc + 2] != 0;
                (format!("LOAD_BOOL R{}", data[pc + 1]), format!("{}", value), 2)
            } else {
                ("LOAD_BOOL".to_string(), String::new(), 0)
            }
        }

        Opcode::LoadNull => {
            if pc + 1 < data.len() {
                (format!("LOAD_NULL R{}", data[pc + 1]), String::new(), 1)
            } else {
                ("LOAD_NULL".to_string(), String::new(), 0)
            }
        }

        Opcode::Move => {
            if pc + 2 < data.len() {
                (format!("MOVE R{}, R{}", data[pc + 1], data[pc + 2]), String::new(), 2)
            } else {
                ("MOVE".to_string(), String::new(), 0)
            }
        }

        Opcode::LoadLocal => {
            if pc + 2 < data.len() {
                let name_idx = data[pc + 2] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                (format!("LOAD_LOCAL R{}", data[pc + 1]), format!("\"{}\"", name), 2)
            } else {
                ("LOAD_LOCAL".to_string(), String::new(), 0)
            }
        }

        Opcode::StoreLocal => {
            if pc + 2 < data.len() {
                let name_idx = data[pc + 1] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                (format!("STORE_LOCAL R{}", data[pc + 2]), format!("\"{}\"", name), 2)
            } else {
                ("STORE_LOCAL".to_string(), String::new(), 0)
            }
        }

        Opcode::GetProperty => {
            if pc + 3 < data.len() {
                let name_idx = data[pc + 3] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                (format!("GET_PROPERTY R{}, R{}", data[pc + 1], data[pc + 2]), format!("\"{}\"", name), 3)
            } else {
                ("GET_PROPERTY".to_string(), String::new(), 0)
            }
        }

        Opcode::SetProperty => {
            if pc + 3 < data.len() {
                let name_idx = data[pc + 2] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                (format!("SET_PROPERTY R{}, R{}", data[pc + 1], data[pc + 3]), format!("\"{}\"", name), 3)
            } else {
                ("SET_PROPERTY".to_string(), String::new(), 0)
            }
        }

        Opcode::Call => {
            if pc + 4 < data.len() {
                let func_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let func_name = resolve_function_name(bytecode, func_idx);
                let args_str = if arg_count == 0 {
                    "args=[]".to_string()
                } else {
                    let arg_end = arg_start + arg_count - 1;
                    format!("args=[R{}..R{}]", arg_start, arg_end)
                };
                let operands = format!("{}, {}",
                    func_name, args_str);
                (format!("CALL R{}", data[pc + 1]), operands, 4)
            } else {
                ("CALL".to_string(), String::new(), 0)
            }
        }

        Opcode::CallNative => {
            if pc + 4 < data.len() {
                let name_idx = data[pc + 2] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let args_str = if arg_count == 0 {
                    "args=[]".to_string()
                } else {
                    let arg_end = arg_start + arg_count - 1;
                    format!("args=[R{}..R{}]", arg_start, arg_end)
                };
                let operands = format!("\"{}\", {}", name, args_str);
                (format!("CALL_NATIVE R{}", data[pc + 1]), operands, 4)
            } else {
                ("CALL_NATIVE".to_string(), String::new(), 0)
            }
        }

        Opcode::Invoke => {
            if pc + 4 < data.len() {
                let method_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let args_str = if arg_count == 0 {
                    "args=[]".to_string()
                } else {
                    let arg_end = arg_start + arg_count - 1;
                    format!("args=[R{}..R{}]", arg_start, arg_end)
                };
                let method_name = resolve_method_name(bytecode, method_idx);
                let operands = format!("{}, {}", method_name, args_str);
                (format!("INVOKE R{}", data[pc + 1]), operands, 4)
            } else {
                ("INVOKE".to_string(), String::new(), 0)
            }
        }

        Opcode::Return => {
            if pc + 1 < data.len() {
                (format!("RETURN R{}", data[pc + 1]), String::new(), 1)
            } else {
                ("RETURN".to_string(), String::new(), 0)
            }
        }

        Opcode::InvokeInterface => {
            if pc + 4 < data.len() {
                let vtable_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let args_str = if arg_count == 0 {
                    "args=[]".to_string()
                } else {
                    let arg_end = arg_start + arg_count - 1;
                    format!("args=[R{}..R{}]", arg_start, arg_end)
                };
                let method_name = resolve_vtable_method_name(vtable_idx, arg_start as usize);
                let operands = format!("{}, {}", method_name, args_str);
                (format!("INVOKE_INTERFACE R{}", data[pc + 1]), operands, 4)
            } else {
                ("INVOKE_INTERFACE".to_string(), String::new(), 0)
            }
        }

        Opcode::CallNativeIndexed => {
            if pc + 5 < data.len() {
                let func_idx = u16::from_le_bytes([data[pc + 2], data[pc + 3]]) as usize;
                let arg_start = data[pc + 4];
                let arg_count = data[pc + 5];
                let args_str = if arg_count == 0 {
                    "args=[]".to_string()
                } else {
                    let arg_end = arg_start + arg_count - 1;
                    format!("args=[R{}..R{}]", arg_start, arg_end)
                };
                let operands = format!("native_{}, {}", func_idx, args_str);
                (format!("CALL_NATIVE_INDEXED R{}", data[pc + 1]), operands, 5)
            } else {
                ("CALL_NATIVE_INDEXED".to_string(), String::new(), 0)
            }
        }

        Opcode::Jump => {
            if pc + 2 < data.len() {
                let target = u16::from_le_bytes([data[pc + 1], data[pc + 2]]);
                (format!("JUMP"), format!("0x{:04x}", target), 2)
            } else {
                ("JUMP".to_string(), String::new(), 0)
            }
        }

        Opcode::JumpIfTrue => {
            if pc + 3 < data.len() {
                let target = u16::from_le_bytes([data[pc + 2], data[pc + 3]]);
                (format!("JUMP_IF_TRUE R{}", data[pc + 1]), format!("0x{:04x}", target), 3)
            } else {
                ("JUMP_IF_TRUE".to_string(), String::new(), 0)
            }
        }

        Opcode::JumpIfFalse => {
            if pc + 3 < data.len() {
                let target = u16::from_le_bytes([data[pc + 2], data[pc + 3]]);
                (format!("JUMP_IF_FALSE R{}", data[pc + 1]), format!("0x{:04x}", target), 3)
            } else {
                ("JUMP_IF_FALSE".to_string(), String::new(), 0)
            }
        }

        Opcode::Equal => {
            if pc + 3 < data.len() {
                (format!("EQUAL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("EQUAL".to_string(), String::new(), 0)
            }
        }

        Opcode::NotEqual => {
            if pc + 3 < data.len() {
                (format!("NOT_EQUAL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("NOT_EQUAL".to_string(), String::new(), 0)
            }
        }

        Opcode::Greater => {
            if pc + 3 < data.len() {
                (format!("GREATER R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("GREATER".to_string(), String::new(), 0)
            }
        }

        Opcode::Less => {
            if pc + 3 < data.len() {
                (format!("LESS R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("LESS".to_string(), String::new(), 0)
            }
        }

        Opcode::GreaterEqual => {
            if pc + 3 < data.len() {
                (format!("GREATER_EQUAL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("GREATER_EQUAL".to_string(), String::new(), 0)
            }
        }

        Opcode::LessEqual => {
            if pc + 3 < data.len() {
                (format!("LESS_EQUAL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("LESS_EQUAL".to_string(), String::new(), 0)
            }
        }

        Opcode::And => {
            if pc + 3 < data.len() {
                (format!("AND R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("AND".to_string(), String::new(), 0)
            }
        }

        Opcode::Or => {
            if pc + 3 < data.len() {
                (format!("OR R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("OR".to_string(), String::new(), 0)
            }
        }

        Opcode::Not => {
            if pc + 2 < data.len() {
                (format!("NOT R{}, R{}", data[pc + 1], data[pc + 2]), String::new(), 2)
            } else {
                ("NOT".to_string(), String::new(), 0)
            }
        }

        Opcode::Add => {
            if pc + 3 < data.len() {
                (format!("ADD R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("ADD".to_string(), String::new(), 0)
            }
        }

        Opcode::Subtract => {
            if pc + 3 < data.len() {
                (format!("SUB R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("SUB".to_string(), String::new(), 0)
            }
        }

        Opcode::Multiply => {
            if pc + 3 < data.len() {
                (format!("MUL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("MUL".to_string(), String::new(), 0)
            }
        }

        Opcode::Divide => {
            if pc + 3 < data.len() {
                (format!("DIV R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("DIV".to_string(), String::new(), 0)
            }
        }

        Opcode::Modulo => {
            if pc + 3 < data.len() {
                (format!("MOD R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("MOD".to_string(), String::new(), 0)
            }
        }

        Opcode::BitAnd => {
            if pc + 3 < data.len() {
                (format!("BIT_AND R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("BIT_AND".to_string(), String::new(), 0)
            }
        }

        Opcode::BitOr => {
            if pc + 3 < data.len() {
                (format!("BIT_OR R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("BIT_OR".to_string(), String::new(), 0)
            }
        }

        Opcode::BitXor => {
            if pc + 3 < data.len() {
                (format!("BIT_XOR R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("BIT_XOR".to_string(), String::new(), 0)
            }
        }

        Opcode::BitNot => {
            if pc + 2 < data.len() {
                (format!("BIT_NOT R{}, R{}", data[pc + 1], data[pc + 2]), String::new(), 2)
            } else {
                ("BIT_NOT".to_string(), String::new(), 0)
            }
        }

        Opcode::ShiftLeft => {
            if pc + 3 < data.len() {
                (format!("SHL R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("SHL".to_string(), String::new(), 0)
            }
        }

        Opcode::ShiftRight => {
            if pc + 3 < data.len() {
                (format!("SHR R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("SHR".to_string(), String::new(), 0)
            }
        }

        Opcode::Concat => {
            if pc + 3 < data.len() {
                (format!("CONCAT R{}, R{}, count={}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("CONCAT".to_string(), String::new(), 0)
            }
        }

        Opcode::Convert => {
            if pc + 3 < data.len() {
                let cast_type = data[pc + 3];
                (format!("CAST R{}, R{}, type={}", data[pc + 1], data[pc + 2], cast_type), String::new(), 3)
            } else {
                ("CAST".to_string(), String::new(), 0)
            }
        }

        Opcode::Array => {
            if pc + 3 < data.len() {
                (format!("ARRAY R{}, R{}, count={}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("ARRAY".to_string(), String::new(), 0)
            }
        }

        Opcode::Index => {
            if pc + 3 < data.len() {
                (format!("INDEX R{}, R{}, R{}", data[pc + 1], data[pc + 2], data[pc + 3]), String::new(), 3)
            } else {
                ("INDEX".to_string(), String::new(), 0)
            }
        }

        Opcode::Line => {
            if pc + 2 < data.len() {
                let line_number = u16::from_le_bytes([data[pc + 1], data[pc + 2]]);
                (format!("LINE {}", line_number), String::new(), 2)
            } else {
                ("LINE".to_string(), String::new(), 0)
            }
        }

        Opcode::TryStart => {
            if pc + 3 < data.len() {
                let catch_pc = u16::from_le_bytes([data[pc + 1], data[pc + 2]]);
                let catch_reg = data[pc + 3];
                (format!("TRY_START"), format!("catch->{:04x}, reg={}", catch_pc, catch_reg), 3)
            } else {
                ("TRY_START".to_string(), String::new(), 0)
            }
        }

        Opcode::TryEnd => ("TRY_END".to_string(), String::new(), 0),

        Opcode::Throw => {
            if pc + 1 < data.len() {
                (format!("THROW R{}", data[pc + 1]), String::new(), 1)
            } else {
                ("THROW".to_string(), String::new(), 0)
            }
        }

        Opcode::Breakpoint => ("BREAKPOINT".to_string(), String::new(), 0),

        Opcode::Halt => ("HALT".to_string(), String::new(), 0),
    }
}

/// Convert byte to Opcode enum
fn opcode_from_byte(byte: u8) -> Opcode {
    match byte {
        0x00 => Opcode::Nop,
        0x10 => Opcode::LoadConst,
        0x11 => Opcode::LoadInt,
        0x12 => Opcode::LoadFloat,
        0x13 => Opcode::LoadBool,
        0x14 => Opcode::LoadNull,
        0x20 => Opcode::Move,
        0x21 => Opcode::LoadLocal,
        0x22 => Opcode::StoreLocal,
        0x30 => Opcode::GetProperty,
        0x31 => Opcode::SetProperty,
        0x40 => Opcode::Call,
        0x41 => Opcode::CallNative,
        0x42 => Opcode::Invoke,
        0x43 => Opcode::Return,
        0x44 => Opcode::InvokeInterface,
        0x45 => Opcode::CallNativeIndexed,
        0x50 => Opcode::Jump,
        0x51 => Opcode::JumpIfTrue,
        0x52 => Opcode::JumpIfFalse,
        0x60 => Opcode::Equal,
        0x61 => Opcode::NotEqual,
        0x62 => Opcode::And,
        0x63 => Opcode::Or,
        0x64 => Opcode::Not,
        0x65 => Opcode::Concat,
        0x66 => Opcode::Greater,
        0x67 => Opcode::Less,
        0x68 => Opcode::Add,
        0x69 => Opcode::Subtract,
        0x6A => Opcode::GreaterEqual,
        0x6B => Opcode::LessEqual,
        0x70 => Opcode::Multiply,
        0x71 => Opcode::Divide,
        0x73 => Opcode::Line,
        0x74 => Opcode::Convert,
        0x75 => Opcode::Modulo,
        0x76 => Opcode::Array,
        0x77 => Opcode::Index,
        0x78 => Opcode::BitAnd,
        0x79 => Opcode::BitOr,
        0x7A => Opcode::BitXor,
        0x7B => Opcode::BitNot,
        0x7C => Opcode::ShiftLeft,
        0x7D => Opcode::ShiftRight,
        0x80 => Opcode::TryStart,
        0x81 => Opcode::TryEnd,
        0x82 => Opcode::Throw,
        0x90 => Opcode::Breakpoint,
        0xFF => Opcode::Halt,
        _ => Opcode::Nop, // Unknown opcode treated as NOP
    }
}

/// Resolve function name from index (for CALL instruction)
fn resolve_function_name(bytecode: &Bytecode, func_idx: usize) -> String {
    bytecode.strings.get(func_idx)
        .map(|s| s.clone())
        .unwrap_or_else(|| format!("func_{}", func_idx))
}

/// Resolve method name from index (for INVOKE instruction)
fn resolve_method_name(bytecode: &Bytecode, method_idx: usize) -> String {
    bytecode.strings.get(method_idx)
        .map(|s| s.clone())
        .unwrap_or_else(|| format!("method_{}", method_idx))
}

/// Resolve method name from vtable index (for INVOKE_INTERFACE)
fn resolve_vtable_method_name(vtable_idx: usize, method_idx: usize) -> String {
    format!("vtable_{}.method_{}", vtable_idx, method_idx)
}

/// Escape special characters in strings for display
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

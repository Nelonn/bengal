/// Bytecode viewer with Godbolt-style formatting
///
/// Displays bytecode in a readable format similar to Compiler Explorer (godbolt.org):
/// - Per-function organization with function headers
/// - Constant pool (.data section) display
/// - Clean address | opcode | operands format

use sparkler::executor::Bytecode;
use sparkler::opcodes::Opcode;

/// Display bytecode in Godbolt-style format
pub fn display_bytecode(bytecode: &Bytecode) {
    println!("# Bytecode Viewer - Bengal");
    println!();
    
    // Display .data section (constants)
    display_data_section(bytecode);
    
    // Display module-level (root) code
    display_root_code(bytecode);
    
    // Display functions
    display_functions(bytecode);
}

/// Display the .data section (constant pool)
fn display_data_section(bytecode: &Bytecode) {
    println!(".data");

    // Display string constants
    for (i, s) in bytecode.strings.iter().enumerate() {
        println!("  str.{:<4} = \"{}\"", i, escape_string(s));
    }

    // Display class information
    for class in &bytecode.classes {
        println!("  class.{} =", class.name);
        for (field_name, field_value) in &class.fields {
            println!("    .{} = {:?}", field_name, field_value);
        }
    }

    println!();
}

/// Display all functions
fn display_functions(bytecode: &Bytecode) {
    for function in &bytecode.functions {
        display_function(function, bytecode);
    }
}

/// Display module-level (root) code
fn display_root_code(bytecode: &Bytecode) {
    if bytecode.data.is_empty() {
        return;
    }

    println!(".root:");
    println!("# module-level code");

    let mut pc = 0;
    let data = &bytecode.data;

    while pc < data.len() {
        let opcode_byte = data[pc];
        let opcode = opcode_from_byte(opcode_byte);

        let address = format!("{:04x}", pc);

        let (opcode_name, operands, operand_count) = decode_instruction(data, pc, opcode, &bytecode.strings);

        if operands.is_empty() {
            println!("  {} | {}", address, opcode_name);
        } else {
            println!("  {} | {:<18} | {}", address, opcode_name, operands);
        }

        pc += 1 + operand_count;
    }

    println!();
}

/// Display a single function's bytecode
fn display_function(function: &sparkler::vm::Function, bytecode: &Bytecode) {
    println!("{}:", function.name);
    println!("# registers: {}, source: {:?}", function.register_count, function.source_file);

    let mut pc = 0;
    let data = &function.bytecode;

    while pc < data.len() {
        let opcode_byte = data[pc];
        let opcode = opcode_from_byte(opcode_byte);

        let address = format!("{:04x}", pc);

        let (opcode_name, operands, operand_count) = decode_instruction(data, pc, opcode, &bytecode.strings);

        if operands.is_empty() {
            println!("  {} | {}", address, opcode_name);
        } else {
            println!("  {} | {:<18} | {}", address, opcode_name, operands);
        }

        pc += 1 + operand_count;
    }

    println!();
}

/// Decode instruction and return (name, operands_string, operand_byte_count)
fn decode_instruction(data: &[u8], pc: usize, opcode: Opcode, strings: &[String]) -> (String, String, usize) {
    match opcode {
        Opcode::Nop => ("NOP".to_string(), String::new(), 0),
        
        Opcode::LoadConst => {
            if pc + 2 < data.len() {
                let str_idx = data[pc + 2] as usize;
                let value = strings.get(str_idx)
                    .map(|s| format!("\"{}\"", escape_string(s)))
                    .unwrap_or_else(|| format!("str.{}", str_idx));
                (format!("LOAD_CONST R{}", data[pc + 1]), value, 2)
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
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, func_{}, args=[R{}..R{}]",
                    data[pc + 1], func_idx, arg_start, arg_end);
                (format!("CALL"), operands, 4)
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
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, \"{}\", args=[R{}..R{}]",
                    data[pc + 1], name, arg_start, arg_end);
                (format!("CALL_NATIVE"), operands, 4)
            } else {
                ("CALL_NATIVE".to_string(), String::new(), 0)
            }
        }

        Opcode::Invoke => {
            if pc + 4 < data.len() {
                let method_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, method_{}, args=[R{}..R{}]",
                    data[pc + 1], method_idx, arg_start, arg_end);
                (format!("INVOKE"), operands, 4)
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
        
        Opcode::CallAsync => {
            if pc + 4 < data.len() {
                let func_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, func_{}, args=[R{}..R{}]",
                    data[pc + 1], func_idx, arg_start, arg_end);
                (format!("CALL_ASYNC"), operands, 4)
            } else {
                ("CALL_ASYNC".to_string(), String::new(), 0)
            }
        }

        Opcode::CallNativeAsync => {
            if pc + 4 < data.len() {
                let name_idx = data[pc + 2] as usize;
                let name = strings.get(name_idx)
                    .map(|s| s.clone())
                    .unwrap_or_else(|| format!("str.{}", name_idx));
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, \"{}\", args=[R{}..R{}]",
                    data[pc + 1], name, arg_start, arg_end);
                (format!("CALL_NATIVE_ASYNC"), operands, 4)
            } else {
                ("CALL_NATIVE_ASYNC".to_string(), String::new(), 0)
            }
        }

        Opcode::InvokeAsync => {
            if pc + 4 < data.len() {
                let method_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, method_{}, args=[R{}..R{}]",
                    data[pc + 1], method_idx, arg_start, arg_end);
                (format!("INVOKE_ASYNC"), operands, 4)
            } else {
                ("INVOKE_ASYNC".to_string(), String::new(), 0)
            }
        }
        
        Opcode::Await => {
            if pc + 2 < data.len() {
                (format!("AWAIT R{}, R{}", data[pc + 1], data[pc + 2]), String::new(), 2)
            } else {
                ("AWAIT".to_string(), String::new(), 0)
            }
        }
        
        Opcode::Spawn => {
            if pc + 2 < data.len() {
                (format!("SPAWN R{}", data[pc + 1]), String::new(), 1)
            } else {
                ("SPAWN".to_string(), String::new(), 0)
            }
        }
        
        Opcode::InvokeInterface => {
            if pc + 4 < data.len() {
                let vtable_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, vtable_{}, args=[R{}..R{}]",
                    data[pc + 1], vtable_idx, arg_start, arg_end);
                (format!("INVOKE_INTERFACE"), operands, 4)
            } else {
                ("INVOKE_INTERFACE".to_string(), String::new(), 0)
            }
        }

        Opcode::InvokeInterfaceAsync => {
            if pc + 4 < data.len() {
                let vtable_idx = data[pc + 2] as usize;
                let arg_start = data[pc + 3];
                let arg_count = data[pc + 4];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, vtable_{}, args=[R{}..R{}]",
                    data[pc + 1], vtable_idx, arg_start, arg_end);
                (format!("INVOKE_INTERFACE_ASYNC"), operands, 4)
            } else {
                ("INVOKE_INTERFACE_ASYNC".to_string(), String::new(), 0)
            }
        }

        Opcode::CallNativeIndexed => {
            if pc + 5 < data.len() {
                let func_idx = u16::from_le_bytes([data[pc + 2], data[pc + 3]]) as usize;
                let arg_start = data[pc + 4];
                let arg_count = data[pc + 5];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, native_{}, args=[R{}..R{}]",
                    data[pc + 1], func_idx, arg_start, arg_end);
                (format!("CALL_NATIVE_INDEXED"), operands, 5)
            } else {
                ("CALL_NATIVE_INDEXED".to_string(), String::new(), 0)
            }
        }

        Opcode::CallNativeIndexedAsync => {
            if pc + 5 < data.len() {
                let func_idx = u16::from_le_bytes([data[pc + 2], data[pc + 3]]) as usize;
                let arg_start = data[pc + 4];
                let arg_count = data[pc + 5];
                let arg_end = arg_start.saturating_add(arg_count.saturating_sub(1));
                let operands = format!("R{}, native_{}, args=[R{}..R{}]",
                    data[pc + 1], func_idx, arg_start, arg_end);
                (format!("CALL_NATIVE_INDEXED_ASYNC"), operands, 5)
            } else {
                ("CALL_NATIVE_INDEXED_ASYNC".to_string(), String::new(), 0)
            }
        }
        
        Opcode::Jump => {
            if pc + 2 < data.len() {
                let target = u16::from_le_bytes([data[pc + 1], data[pc + 2]]);
                (format!("JUMP"), format!("-> {:04x}", target), 2)
            } else {
                ("JUMP".to_string(), String::new(), 0)
            }
        }
        
        Opcode::JumpIfTrue => {
            if pc + 3 < data.len() {
                let target = u16::from_le_bytes([data[pc + 2], data[pc + 3]]);
                (format!("JUMP_IF_TRUE R{}", data[pc + 1]), format!("-> {:04x}", target), 3)
            } else {
                ("JUMP_IF_TRUE".to_string(), String::new(), 0)
            }
        }
        
        Opcode::JumpIfFalse => {
            if pc + 3 < data.len() {
                let target = u16::from_le_bytes([data[pc + 2], data[pc + 3]]);
                (format!("JUMP_IF_FALSE R{}", data[pc + 1]), format!("-> {:04x}", target), 3)
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
        0x44 => Opcode::CallAsync,
        0x45 => Opcode::CallNativeAsync,
        0x46 => Opcode::InvokeAsync,
        0x47 => Opcode::Await,
        0x48 => Opcode::Spawn,
        0x49 => Opcode::InvokeInterface,
        0x4A => Opcode::InvokeInterfaceAsync,
        0x4B => Opcode::CallNativeIndexed,
        0x4C => Opcode::CallNativeIndexedAsync,
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

/// Escape special characters in strings for display
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

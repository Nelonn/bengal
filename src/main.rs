use bengal_compiler::Compiler;
use sparkler::Executor;
use std::env;
use std::fs;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <source_file> [--dump-bytecode]", args[0]);
        std::process::exit(1);
    }

    let source_file = &args[1];
    let dump_bytecode = args.iter().any(|arg| arg == "--dump-bytecode");
    let debug_mode = args.iter().any(|arg| arg == "--debug");

    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let mut compiler = Compiler::with_path(&source, source_file);
    let bytecode = match compiler.compile() {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("Compilation error: {}", e);
            std::process::exit(1);
        }
    };

    if dump_bytecode {
        println!("--- BYTECODE DUMP ---");
        println!("Bytecode data ({} bytes):", bytecode.data.len());
        let mut i = 0;
        while i < bytecode.data.len() {
            let byte = bytecode.data[i];
            let name = get_opcode_name(byte);
            print!("{:04X}: 0x{:02X} ({})", i, byte, name);
            
            // Basic operand display for some common opcodes
            match byte {
                0x10 | 0x20 | 0x21 | 0x30 | 0x31 | 0x40 | 0x41 | 0x43 | 0x44 | 0x45 | 0x50 | 0x51 | 0x52 | 0x65 => {
                    if i + 1 < bytecode.data.len() {
                        i += 1;
                        print!(" operand: 0x{:02X}", bytecode.data[i]);
                    }
                }
                0x55 | 0x56 | 0x57 | 0x73 | 0x80 => {
                    // 2-byte operands (u16)
                    if i + 2 < bytecode.data.len() {
                        let low = bytecode.data[i + 1];
                        let high = bytecode.data[i + 2];
                        let val = u16::from_le_bytes([low, high]);
                        print!(" operand: 0x{:04X} ({})", val, val);
                        i += 2;
                    }
                }
                0x42 | 0x46 => {
                    // 2 operands: name_idx and arg_count
                    if i + 1 < bytecode.data.len() {
                        i += 1;
                        print!(" operand1: 0x{:02X}", bytecode.data[i]);
                    }
                    if i + 1 < bytecode.data.len() {
                        i += 1;
                        print!(" operand2: 0x{:02X}", bytecode.data[i]);
                    }
                }
                0x11 | 0x12 => {
                    // 8-byte operands
                    print!(" operands:");
                    for _ in 0..8 {
                        if i + 1 < bytecode.data.len() {
                            i += 1;
                            print!(" 0x{:02X}", bytecode.data[i]);
                        }
                    }
                }
                _ => {}
            }
            println!();
            i += 1;
        }

        println!("\nStrings table ({} entries):", bytecode.strings.len());
        for (i, s) in bytecode.strings.iter().enumerate() {
            println!("  [{}] \"{}\"", i, s);
        }
        println!("--- END DUMP ---");
        return;
    }

    let mut executor = Executor::new();
    bengal_std::register_all(&mut executor.vm);

    if debug_mode {
        executor.vm.is_debugging = true;
        // For testing, add a breakpoint at line 3 of the source file
        executor.vm.breakpoints.insert((source_file.clone(), 3));
    }

    if let Err(e) = executor.run_to_completion(bytecode, Some(source_file)).await {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}

fn get_opcode_name(op: u8) -> &'static str {
    match op {
        0x00 => "Nop",
        0x10 => "LoadConst",
        0x11 => "LoadInt",
        0x12 => "LoadFloat",
        0x13 => "LoadBool",
        0x14 => "LoadNull",
        0x20 => "Move",
        0x21 => "LoadLocal",
        0x22 => "StoreLocal",
        0x30 => "GetProperty",
        0x31 => "SetProperty",
        0x40 => "Call",
        0x41 => "CallNative",
        0x42 => "Invoke",
        0x43 => "Return",
        0x44 => "CallAsync",
        0x45 => "CallNativeAsync",
        0x46 => "InvokeAsync",
        0x47 => "Await",
        0x48 => "Spawn",
        0x50 => "Jump",
        0x51 => "JumpIfTrue",
        0x52 => "JumpIfFalse",
        0x60 => "Equal",
        0x61 => "NotEqual",
        0x62 => "And",
        0x63 => "Or",
        0x64 => "Not",
        0x65 => "Concat",
        0x66 => "Greater",
        0x67 => "Less",
        0x68 => "Add",
        0x69 => "Subtract",
        0x70 => "Multiply",
        0x71 => "Divide",
        0x73 => "Line",
        0x74 => "Cast",
        0x75 => "Modulo",
        0x80 => "TryStart",
        0x81 => "TryEnd",
        0x82 => "Throw",
        0x90 => "Breakpoint",
        0xFF => "Halt",
        _ => "Unknown",
    }
}

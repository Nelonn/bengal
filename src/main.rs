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

    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let mut compiler = Compiler::new(&source);
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

    if let Err(e) = executor.run_to_completion(bytecode, Some(source_file)).await {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}

fn get_opcode_name(op: u8) -> &'static str {
    match op {
        0x00 => "Nop",
        0x10 => "PushString",
        0x11 => "PushInt",
        0x12 => "PushFloat",
        0x13 => "PushBool",
        0x14 => "PushNull",
        0x20 => "LoadLocal",
        0x21 => "StoreLocal",
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
        0x53 => "JumpIfGreater",
        0x54 => "JumpIfLess",
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
        0x72 => "Pop",
        0x80 => "TryStart",
        0x81 => "TryEnd",
        0x82 => "Throw",
        0xFF => "Halt",
        _ => "Unknown",
    }
}

use bengal_compiler::Compiler;
use sparkler::Executor;
use std::fs;
use std::path::{Path, PathBuf};
use clap::Parser;

mod repl;

async fn run_file(source_file: &str, debug: bool, unsafe_fast: bool) -> Result<(), Box<dyn std::error::Error>> {
    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            return Err(format!("Error reading file: {}", e).into());
        }
    };

    let mut compiler = Compiler::with_path_and_options(&source, source_file, unsafe_fast);
    compiler.enable_type_checking = false;
    let bytecode = match compiler.compile() {
        Ok(bc) => bc,
        Err(e) => {
            return Err(format!("Compilation error: {}", e).into());
        }
    };

    let mut executor = Executor::new();
    bengal_std::register_all(&mut executor.vm);

    if debug {
        executor.vm.is_debugging = true;
        // For testing, add a breakpoint at line 3 of the source file
        executor.vm.breakpoints.insert((source_file.to_string(), 3));
    }

    if let Err(e) = executor.run_to_completion(bytecode, Some(source_file)).await {
        return Err(format!("Runtime error: {}", e).into());
    }

    Ok(())
}

async fn run_tests(test_path: &str, unsafe_fast: bool) -> Result<(), Box<dyn std::error::Error>> {
    let path = Path::new(test_path);
    let mut files_to_test = Vec::new();

    if path.is_file() {
        files_to_test.push(path.to_path_buf());
    } else if path.is_dir() {
        find_test_files(path, &mut files_to_test)?;
    } else {
        return Err(format!("Path not found: {}", test_path).into());
    }

    if files_to_test.is_empty() {
        println!("No test files found.");
        return Ok(());
    }

    println!("Running {} test file(s)...", files_to_test.len());
    let mut passed = 0;
    let mut failed = 0;

    for file in files_to_test {
        let file_name = file.to_string_lossy();
        print!("Testing: {}... ", file_name);
        std::io::Write::flush(&mut std::io::stdout())?;

        match run_file(&file_name, false, unsafe_fast).await {
            Ok(_) => {
                println!("PASS");
                passed += 1;
            }
            Err(e) => {
                println!("FAIL");
                eprintln!("  Error: {}", e);
                failed += 1;
            }
        }
    }

    println!("\nTest Summary:");
    println!("  Total:  {}", passed + failed);
    println!("  Passed: {}", passed);
    println!("  Failed: {}", failed);

    if failed > 0 {
        std::process::exit(1);
    }

    Ok(())
}

fn find_test_files(dir: &Path, files: &mut Vec<PathBuf>) -> std::io::Result<()> {
    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                find_test_files(&path, files)?;
            } else if path.is_file() {
                if let Some(file_name) = path.file_name().and_then(|n| n.to_str()) {
                    if file_name.starts_with("test_") || file_name.ends_with("_test.bl") {
                        files.push(path);
                    }
                }
            }
        }
    }
    Ok(())
}

#[derive(Parser, Debug)]
#[command(name = "bengal")]
#[command(about = "Bengal Language CLI", long_about = None)]
struct Args {
    /// Source file to run (omit to enter REPL mode)
    source_file: Option<String>,

    /// Run tests in the specified file or directory
    #[arg(long)]
    test: Option<String>,

    /// Dump bytecode information
    #[arg(long)]
    dump_bytecode: bool,

    /// Enable debug mode with breakpoints
    #[arg(long)]
    debug: bool,

    /// Disable safety checks (overflow, division by zero) for faster execution
    #[arg(long)]
    unsafe_fast: bool,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Some(test_path) = args.test {
        if let Err(e) = run_tests(&test_path, args.unsafe_fast).await {
            eprintln!("Testing error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // REPL mode - default when no source file is provided
    if args.source_file.is_none() {
        if let Err(e) = repl::run_repl().await {
            eprintln!("REPL error: {}", e);
            std::process::exit(1);
        }
        return;
    }

    // File execution mode
    let source_file = args.source_file.unwrap();

    if args.dump_bytecode {
        let source = match fs::read_to_string(&source_file) {
            Ok(content) => content,
            Err(e) => {
                eprintln!("Error reading file: {}", e);
                std::process::exit(1);
            }
        };

        let mut compiler = Compiler::with_path_and_options(&source, &source_file, args.unsafe_fast);
        compiler.enable_type_checking = false;
        let bytecode = match compiler.compile() {
            Ok(bc) => bc,
            Err(e) => {
                eprintln!("Compilation error: {}", e);
                std::process::exit(1);
            }
        };

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

    if let Err(e) = run_file(&source_file, args.debug, args.unsafe_fast).await {
        eprintln!("{}", e);
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
        0x49 => "InvokeInterface",
        0x4A => "InvokeInterfaceAsync",
        0x4B => "CallNativeIndexed",
        0x4C => "CallNativeIndexedAsync",
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

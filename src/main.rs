use bengal_compiler::{HlirCompiler, CompilerOptions, sparkler_to_bytecode};
use sparkler::Executor;
use bytecode_viewer;
use std::fs;
use std::path::{Path, PathBuf};
use clap::Parser;

mod repl;

async fn run_file(source_file: &str, debug: bool, script_args: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            return Err(format!("Error reading file: {}", e).into());
        }
    };

    let options = CompilerOptions {
        enable_type_checking: true,
        search_paths: vec!["std".to_string()],
        emit_llvm_ir: false,
        emit_sparkler_bytecode: true,
    };

    let mut compiler = HlirCompiler::with_path_and_options(&source, source_file, options);
    let result = match compiler.compile() {
        Ok(r) => r,
        Err(e) => {
            return Err(format!("Compilation error: {}", e).into());
        }
    };

    let bytecode = sparkler_to_bytecode(
        result.sparkler_bytecode.ok_or("Bytecode generation failed")?
    );

    let mut executor = Executor::with_linker();
    bengal_std::register_all(&mut executor.vm);

    // Sync the linker's registry with the VM's registry
    // This is needed because register_all registers with the VM directly,
    // but the linker needs to have the same registry for bytecode linking
    if let Some(linker) = executor.linker.as_mut() {
        let registry = linker.registry();
        *registry.write().unwrap() = executor.vm.program.native_registry.clone();
    }

    // Pass arguments to the script as ARGV
    use std::sync::{Arc, Mutex};
    executor.vm.set_local("ARGV", sparkler::Value::Array(Arc::new(Mutex::new(
        script_args.iter()
            .map(|s| sparkler::Value::String(s.clone()))
            .collect()
    ))));

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

async fn run_tests(test_path: &str) -> Result<(), Box<dyn std::error::Error>> {
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

        match run_file(&file_name, false, Vec::new()).await {
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

    /// Arguments to pass to the script
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    script_args: Vec<String>,
}

#[tokio::main]
async fn main() {
    let args = Args::parse();

    if let Some(test_path) = args.test {
        if let Err(e) = run_tests(&test_path).await {
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

        let options = CompilerOptions {
            enable_type_checking: false,
            search_paths: vec!["std".to_string()],
            emit_llvm_ir: false,
            emit_sparkler_bytecode: true,
        };
        
        let mut compiler = HlirCompiler::with_path_and_options(&source, &source_file, options);
        let result = match compiler.compile() {
            Ok(r) => r,
            Err(e) => {
                eprintln!("Compilation error: {}", e);
                std::process::exit(1);
            }
        };
        
        let bytecode = sparkler_to_bytecode(
            result.sparkler_bytecode.unwrap()
        );

        bytecode_viewer::display_bytecode(&bytecode);
        return;
    }

    if let Err(e) = run_file(&source_file, args.debug, args.script_args).await {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

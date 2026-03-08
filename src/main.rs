use bengal_compiler::Compiler;
use sparkler::Executor;
use std::env;
use std::fs;

#[tokio::main]
async fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        eprintln!("Usage: {} <source_file>", args[0]);
        std::process::exit(1);
    }

    let source_file = &args[1];

    let source = match fs::read_to_string(source_file) {
        Ok(content) => content,
        Err(e) => {
            eprintln!("Error reading file: {}", e);
            std::process::exit(1);
        }
    };

    let compiler = Compiler::new(&source);
    let bytecode = match compiler.compile() {
        Ok(bc) => bc,
        Err(e) => {
            eprintln!("Compilation error: {}", e);
            std::process::exit(1);
        }
    };

    let mut executor = Executor::new();
    if let Err(e) = executor.run_to_completion(bytecode).await {
        eprintln!("Runtime error: {}", e);
        std::process::exit(1);
    }
}

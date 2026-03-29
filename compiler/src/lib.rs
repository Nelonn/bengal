pub mod lexer;
pub mod parser;
pub mod types;
pub mod resolver;
pub mod hlir;
pub mod ast_to_hlir_full;
pub mod hlir_to_sparkler;
pub mod hlir_compiler;
#[cfg(feature = "llvm")]
pub mod llvm;

pub use hlir::{HlirBuilder, HlirModule, HlirType, HlirValue, HlirBinOp, HlirUnaryOp};
pub use ast_to_hlir_full::{AstToHlirConverter, ast_to_hlir};
pub use hlir_to_sparkler::{HlirToSparkler, CompiledBytecode, compile_hlir_to_sparkler};
pub use hlir_compiler::{HlirCompiler, CompilerOptions, CompilationResult, sparkler_to_bytecode};
#[cfg(feature = "llvm")]
pub use llvm::{LlvmBackend, LlvmIrGenerator};

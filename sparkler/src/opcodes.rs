/// Opcodes for the Bengal VM
///
/// The opcode design follows these principles:
/// - Compact encoding: Frequently used opcodes have shorter encodings
/// - Fixed register file per call frame
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Opcode {
    Nop = 0x00,

    // Load constants into registers
    LoadConst = 0x10,  // Rd, string_idx
    LoadInt = 0x11,    // Rd, 8 bytes
    LoadFloat = 0x12,  // Rd, 8 bytes
    LoadBool = 0x13,   // Rd, 1 byte
    LoadNull = 0x14,   // Rd

    // Register-to-register operations
    Move = 0x20,       // Rd, Rs

    // Local variable operations
    LoadLocal = 0x21,  // Rd, name_idx
    StoreLocal = 0x22, // name_idx, Rs

    // Property access
    GetProperty = 0x30,  // Rd, Robj, name_idx
    SetProperty = 0x31,  // Robj, name_idx, Rs

    // Function calls
    Call = 0x40,         // Rd, func_idx, arg_start, arg_count
    CallNative = 0x41,   // Rd, name_idx, arg_start, arg_count
    Invoke = 0x42,       // Rd, method_idx, arg_start, arg_count
    Return = 0x43,       // Rs
    CallAsync = 0x44,
    CallNativeAsync = 0x45,
    InvokeAsync = 0x46,
    Await = 0x47,
    Spawn = 0x48,
    InvokeInterface = 0x49,  // Rd, vtable_idx, arg_start, arg_count
    InvokeInterfaceAsync = 0x4A,  // Rd, vtable_idx, arg_start, arg_count

    // Indexed native calls (optimized - uses function index instead of string lookup)
    CallNativeIndexed = 0x4B,  // Rd, func_idx (u16), arg_start, arg_count
    CallNativeIndexedAsync = 0x4C,  // Rd, func_idx (u16), arg_start, arg_count

    // Control flow
    Jump = 0x50,         // target (2 bytes)
    JumpIfTrue = 0x51,   // Rs, target (2 bytes)
    JumpIfFalse = 0x52,  // Rs, target (2 bytes)

    // Comparisons (3-register format: Rd = Rs1 op Rs2)
    Equal = 0x60,    // Rd, Rs1, Rs2
    NotEqual = 0x61, // Rd, Rs1, Rs2
    Greater = 0x66,  // Rd, Rs1, Rs2
    Less = 0x67,     // Rd, Rs1, Rs2
    GreaterEqual = 0x6A,
    LessEqual = 0x6B,

    // Logical operations
    And = 0x62,      // Rd, Rs1, Rs2
    Or = 0x63,       // Rd, Rs1, Rs2
    Not = 0x64,      // Rd, Rs

    // Arithmetic (3-register format)
    Add = 0x68,      // Rd, Rs1, Rs2
    Subtract = 0x69, // Rd, Rs1, Rs2
    Multiply = 0x70, // Rd, Rs1, Rs2
    Divide = 0x71,   // Rd, Rs1, Rs2
    Modulo = 0x75,   // Rd, Rs1, Rs2

    // String operations
    Concat = 0x65,   // Rd, rs_start, count

    // Type operations
    Convert = 0x74,  // Rd, Rs, type
    Array = 0x76,    // Rd, rs_start, count
    Index = 0x77,    // Rd, Robj, Ridx

    // Debugging
    Line = 0x73,     // line_number (2 bytes)

    // Exception handling
    TryStart = 0x80, // catch_pc (2 bytes), catch_reg
    TryEnd = 0x81,
    Throw = 0x82,    // Rs

    // Debugging
    Breakpoint = 0x90,

    // Execution control
    Halt = 0xFF,
}

impl Opcode {
    /// Get the number of bytes for each opcode (including the opcode byte itself)
    pub fn size(&self) -> usize {
        match self {
            Opcode::Nop => 1,
            Opcode::LoadConst => 3,
            Opcode::LoadInt => 9,
            Opcode::LoadFloat => 9,
            Opcode::LoadBool => 2,
            Opcode::LoadNull => 2,
            Opcode::Move => 3,
            Opcode::LoadLocal => 3,
            Opcode::StoreLocal => 3,
            Opcode::GetProperty => 4,
            Opcode::SetProperty => 4,
            Opcode::Call => 5,
            Opcode::CallNative => 5,
            Opcode::Invoke => 5,
            Opcode::Return => 2,
            Opcode::CallAsync => 5,
            Opcode::CallNativeAsync => 5,
            Opcode::InvokeAsync => 5,
            Opcode::Await => 3,
            Opcode::Spawn => 3,
            Opcode::InvokeInterface => 5,
            Opcode::InvokeInterfaceAsync => 5,
            Opcode::CallNativeIndexed => 6,
            Opcode::CallNativeIndexedAsync => 6,
            Opcode::Jump => 3,
            Opcode::JumpIfTrue => 4,
            Opcode::JumpIfFalse => 4,
            Opcode::Equal => 4,
            Opcode::NotEqual => 4,
            Opcode::Greater => 4,
            Opcode::Less => 4,
            Opcode::GreaterEqual => 4,
            Opcode::LessEqual => 4,
            Opcode::And => 4,
            Opcode::Or => 4,
            Opcode::Not => 3,
            Opcode::Add => 4,
            Opcode::Subtract => 4,
            Opcode::Multiply => 4,
            Opcode::Divide => 4,
            Opcode::Modulo => 4,
            Opcode::Concat => 4,
            Opcode::Convert => 4,
            Opcode::Array => 4,
            Opcode::Index => 4,
            Opcode::Line => 3,
            Opcode::TryStart => 4,
            Opcode::TryEnd => 1,
            Opcode::Throw => 2,
            Opcode::Breakpoint => 1,
            Opcode::Halt => 1,
        }
    }
}

pub mod bytecode;
pub mod document;
pub mod ladder;
pub mod il_format;
pub use il_format as ld_format;
pub mod shape;
pub mod simulation;

pub use bytecode::{
    BytecodeCompiler, BytecodeInstruction, BytecodeProgram, CompileReport, HardwareIoConfig,
    MemoryMapEntry, MemoryRegion, PlcSerialCommand, PlcStatusSnapshot, VmOpcode,
    IO_CONFIG_PACKET_LEN, IO_CONFIG_PAYLOAD_LEN, IO_FLAG_INPUT_ACTIVE_LOW, IO_FLAG_INPUT_PULL_UP,
    IO_FLAG_STATUS_LED_HEARTBEAT, PACKET_SOF, PIN_UNASSIGNED,
};
pub use document::Document;
pub use ladder::{
    parse_int_literal, CoilMode, CoilStyle, ContactMode, ContactStyle, FbParams, GlobalSymbolTable, IOPortMapping,
    IecVarEntry, InsertPosition, LadderElement, LadderProgram, Rung, SchematicStyle, SymbolRecord, TypeTag,
};
pub use il_format::{IlParser, LdParser};
pub use shape::{ColorRgb, Point2D, Shape, ShapeKind};
pub use simulation::{PlcSimulator, SimState};


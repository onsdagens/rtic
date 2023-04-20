#[cfg(not(any(feature = "riscv-e310x-backend")))]
compile_error!("Building for RISC-V SLIC, but 'riscv-e310x-backen' backend not selected");

/// GENERIC RE-EXPORTS: needed for all RTIC backends
#[cfg(feature = "riscv-e310x-backend")]
pub use e310x::Peripherals; // TODO is this REALLY necessary? Can we move it to macros/make it optional?
pub use riscv_slic::{lock, pend, run, swi::InterruptNumber};

/// USE CASE RE-EXPORTS: needed for SLIC-only
pub use riscv_slic::{self, clear_interrupts, codegen, set_interrupts, set_priority};

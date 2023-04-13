pub use bare_metal::CriticalSection;
//pub use portable_atomic as atomic;
pub use atomic_polyfill as atomic;

pub mod executor;

#[cfg(all(
    feature = "cortex-m-basepri",
    not(any(feature = "thumbv7-backend", feature = "thumbv8main-backend"))
))]
compile_error!(
    "Building for Cortex-M with basepri, but 'thumbv7-backend' or 'thumbv8main-backend' backend not selected"
);

#[cfg(all(
    feature = "cortex-m-source-masking",
    not(any(feature = "thumbv6-backend", feature = "thumbv8base-backend"))
))]
compile_error!(
    "Building for Cortex-M with source masking, but 'thumbv6-backend' or 'thumbv8base-backend' backend not selected"
);

#[cfg(all(feature = "riscv-slic", not(any(feature = "riscv-slic-backend"))))]
compile_error!("Building for RISC-V SLIC, but 'riscv-slic-backend' backend not selected");

#[cfg(any(feature = "cortex-m-basepri", feature = "rtic-uitestv7"))]
pub use cortex_basepri::*;

#[cfg(any(feature = "cortex-m-basepri", feature = "rtic-uitestv7"))]
mod cortex_basepri;

#[cfg(any(feature = "cortex-m-source-masking", feature = "rtic-uitestv6"))]
pub use cortex_source_mask::*;

#[cfg(any(feature = "cortex-m-source-masking", feature = "rtic-uitestv6"))]
mod cortex_source_mask;

/// Priority conversion, takes logical priorities 1..=N and converts it to NVIC priority.
#[cfg(feature = "cortex-m")]
#[inline]
#[must_use]
pub const fn cortex_logical2hw(logical: u8, nvic_prio_bits: u8) -> u8 {
    ((1 << nvic_prio_bits) - logical) << (8 - nvic_prio_bits)
}

#[cfg(feature = "cortex-m")]
use cortex_m::{interrupt::InterruptNumber, peripheral::NVIC};

/// Sets the given `interrupt` as pending
///
/// This is a convenience function around
/// [`NVIC::pend`](../cortex_m/peripheral/struct.NVIC.html#method.pend)
#[cfg(feature = "cortex-m")]
pub fn pend<I>(interrupt: I)
where
    I: InterruptNumber,
{
    NVIC::pend(interrupt);
}

#[cfg(feature = "riscv-slic")]
pub use self::riscv_slic::*;

#[cfg(feature = "riscv-slic")]
mod riscv_slic;

#[inline(always)]
pub fn assert_send<T>()
where
    T: Send,
{
}

#[inline(always)]
pub fn assert_sync<T>()
where
    T: Sync,
{
}

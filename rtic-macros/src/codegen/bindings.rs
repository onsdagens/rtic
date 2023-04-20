#[cfg(any(feature = "cortex-m-source-masking", feature = "cortex-m-basepri"))]
pub use cortex::*;

#[cfg(feature = "riscv-slic")]
pub use self::riscv_slic::*;

#[cfg(feature = "test-template")]
pub use template::*;

#[cfg(any(feature = "cortex-m-source-masking", feature = "cortex-m-basepri"))]
mod cortex;

#[cfg(feature = "riscv-slic")]
mod riscv_slic;

#[cfg(feature = "test-template")]
mod template;

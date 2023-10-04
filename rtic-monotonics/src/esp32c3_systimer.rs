//! [`Monotonic`] based on ESP32C3 SYSTIMER.
//!

//! # Example
//!
//! ```
//! use rtic_monotonics::systimer::*;
//!
//! fn init() {
//!     # // This is normally provided by the selected PAC
//!     # let systimer = unsafe { core::mem::transmute(()) };
//!     // Generate the required token
//!     let systimer_token = rtic_monotonics::create_systimer_token!();
//!
//!     // Start the monotonic
//!     Systimer::start(systimer, systimer_token);
//! }
//!
//! async fn usage() {
//!     loop {
//!          // Use the monotonic
//!          Systimer::delay(100.millis()).await;
//!     }
//! }
//! ```

use super::Monotonic;
pub use super::{TimeoutError, TimerQueue};
use core::future::Future;
use esp32c3::{INTERRUPT_CORE0, SYSTIMER};
pub use fugit::{self, ExtU64};

//Timer runs at 16MHz by default
const TIMER_HZ: u32 = 16_000_000;

/// Systimer implementing monotonic
pub struct Systimer;

impl Systimer {
    /// Start a `Monotonic` based on SysTimer.
    ///
    /// Note: Give the return value to `TimerQueue::initialize()` to initialize the timer queue.
    pub fn start(systimer: SYSTIMER, _interrupt_token: impl crate::InterruptToken<Self>) {
        const INTERRUPT_MAP_BASE: u32 = 0x600c2000;
        let interrupt_number = 37 as isize;
        let cpu_interrupt_number = 31 as isize;
        unsafe {
            let intr_map_base = INTERRUPT_MAP_BASE as *mut u32;
            intr_map_base
                .offset(interrupt_number)
                .write_volatile(cpu_interrupt_number as u32);
            //map peripheral interrupt to CPU interrupt
            (*INTERRUPT_CORE0::ptr())
                .cpu_int_enable
                .modify(|r, w| w.bits((1 << cpu_interrupt_number) | r.bits())); //enable the CPU interupt.
            let intr = INTERRUPT_CORE0::ptr();
            let intr_prio_base = (*intr).cpu_int_pri_0.as_ptr();

            intr_prio_base
                .offset(cpu_interrupt_number)
                .write_volatile(15 as u32);
        }

        SYSTIMER_TIMER_QUEUE.initialize(Systimer {});
    }
}

static SYSTIMER_TIMER_QUEUE: TimerQueue<Systimer> = TimerQueue::new();

// Forward timerqueue interface
impl Systimer {
    /// Used to access the underlying timer queue
    #[doc(hidden)]
    pub fn __tq() -> &'static TimerQueue<Systimer> {
        &SYSTIMER_TIMER_QUEUE
    }

    /// Timeout at a specific time.
    pub async fn timeout_at<F: Future>(
        instant: <Self as Monotonic>::Instant,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        SYSTIMER_TIMER_QUEUE.timeout_at(instant, future).await
    }

    /// Timeout after a specific duration.
    #[inline]
    pub async fn timeout_after<F: Future>(
        duration: <Self as Monotonic>::Duration,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        SYSTIMER_TIMER_QUEUE.timeout_after(duration, future).await
    }

    /// Delay for some duration of time.
    #[inline]
    pub async fn delay(duration: <Self as Monotonic>::Duration) {
        SYSTIMER_TIMER_QUEUE.delay(duration).await;
    }

    /// Delay to some specific time instant.
    #[inline]
    pub async fn delay_until(instant: <Self as Monotonic>::Instant) {
        SYSTIMER_TIMER_QUEUE.delay_until(instant).await;
    }
}

impl Monotonic for Systimer {
    type Instant = fugit::TimerInstantU64<TIMER_HZ>;
    type Duration = fugit::TimerDurationU64<TIMER_HZ>;
    const ZERO: Self::Instant = Self::Instant::from_ticks(0);
    fn now() -> Self::Instant {
        let peripherals = unsafe { esp32c3::Peripherals::steal() };
        peripherals
            .SYSTIMER
            .unit0_op
            .write(|w| w.timer_unit0_update().set_bit()); //load the timer
        let instant: u64 = (peripherals.SYSTIMER.unit0_value_lo.read().bits() as u64)
            | ((peripherals.SYSTIMER.unit0_value_hi.read().bits() as u64) << 32);
        Self::Instant::from_ticks(instant)
    }

    fn set_compare(instant: Self::Instant) {
        let systimer = unsafe { esp32c3::Peripherals::steal() }.SYSTIMER;
        systimer
            .target0_conf
            .write(|w| w.target0_timer_unit_sel().set_bit());
        let now = Self::now();
        systimer
            .target0_conf
            .write(|w| w.target0_period_mode().clear_bit());
        //value, 16MHZ/16k = 1kHz
        systimer
            .target0_lo
            .write(|w| unsafe { w.bits(((instant.ticks()) & 0xFFFFFFFF).try_into().unwrap()) });
        systimer
            .target0_hi
            .write(|w| unsafe { w.bits(((instant.ticks()) >> 32).try_into().unwrap()) });
        systimer
            .comp0_load
            .write(|w| w.timer_comp0_load().set_bit()); //sync period to comp register
        systimer.conf.write(|w| w.target0_work_en().set_bit()); //enable comparisons
        systimer.int_ena.write(|w| w.target0_int_ena().set_bit()); //enable interrupts
    }

    fn clear_compare_flag() {
        use esp32c3::Peripherals;
        unsafe { Peripherals::steal() }
            .SYSTIMER
            .int_clr
            .write(|w| w.target0_int_clr().set_bit());
    }

    fn pend_interrupt() {
        extern "C" {
            fn cpu_int_31_handler();
        }
        //run the timer interrupt handler in a critical section to emulate a max priority
        //interrupt.
        //since there is no hardware support for pending a timer interrupt.
        unsafe { riscv::interrupt::disable() };
        unsafe { cpu_int_31_handler() };
        unsafe { riscv::interrupt::enable() };
    }

    fn on_interrupt() {}

    fn enable_timer() {}

    fn disable_timer() {}
}

#[cfg(feature = "embedded-hal-async")]
impl embedded_hal_async::delay::DelayUs for Systimer {
    async fn delay_us(&mut self, us: u32) {
        Self::delay(us.micros()).await;
    }

    async fn delay_ms(&mut self, ms: u32) {
        Self::delay(ms.millis()).await;
    }
}

/// Register the Systimer interrupt for the monotonic.
#[macro_export]
macro_rules! create_systimer_token {
    () => {{
        #[export_name="cpu_int_31_handler"]
        #[allow(non_snake_case)]
        unsafe extern "C" fn Systimer() {
            rtic_monotonics::esp32c3_systimer::Systimer::__tq().on_monotonic_interrupt();
        }
        pub struct SystimerToken;

        unsafe impl $crate::InterruptToken<rtic_monotonics::esp32c3_systimer::Systimer> for SystimerToken {}

        SystimerToken
    }};
}

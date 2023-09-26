//! [`Monotonic`] based on Cortex-M SysTick. Note: this implementation is inefficient as it
//! ticks and generates interrupts at a constant rate.
//!
//! Currently, the following tick rates are supported:
//!
//! | Feature          | Tick rate | Precision |
//! |:----------------:|----------:|----------:|
//! | (none / default) |  1 Hz     |      1 ms |
//! |   systick-100hz  | 100 Hz    |     10 ms |
//! |   systick-10khz  | 10 KHz    |    0.1 ms |

//! # Example
//!
//! ```
//! use rtic_monotonics::systick::*;
//!
//! fn init() {
//!     # // This is normally provided by the selected PAC
//!     # let systick = unsafe { core::mem::transmute(()) };
//!     // Generate the required token
//!     let systick_token = rtic_monotonics::create_systick_token!();
//!
//!     // Start the monotonic
//!     Systick::start(systick, 12_000_000, systick_token);
//! }
//!
//! async fn usage() {
//!     loop {
//!          // Use the monotonic
//!          Systick::delay(100.millis()).await;
//!     }
//! }
//! ```

use super::Monotonic;
pub use super::{TimeoutError, TimerQueue};
use atomic_polyfill::AtomicU32;
use core::future::Future;
use esp32c3::{SYSTIMER, INTERRUPT_CORE0};
pub use fugit::{self, ExtU32};

// Features should be additive, here systick-100hz gets picked if both
// `systick-100hz` and `systick-10khz` are enabled.

const TIMER_HZ: u32 = 100;
    


/// Systick implementing `rtic_monotonic::Monotonic` which runs at 1 kHz, 100Hz or 10 kHz.
pub struct Systick;

impl Systick {
    /// Start a `Monotonic` based on SysTick.
    ///
    /// The `sysclk` parameter is the speed at which SysTick runs at. This value should come from
    /// the clock generation function of the used HAL.
    ///
    /// Notice that the actual rate of the timer is a best approximation based on the given
    /// `sysclk` and `TIMER_HZ`.
    ///
    /// Note: Give the return value to `TimerQueue::initialize()` to initialize the timer queue.
    pub fn start(
        mut systick: esp32c3::SYSTIMER,
        sysclk: u32,
        _interrupt_token: impl crate::InterruptToken<Self>,
    ) {
        // + TIMER_HZ / 2 provides round to nearest instead of round to 0.
        // - 1 as the counter range is inclusive [0, reload]
        let reload = (sysclk + TIMER_HZ / 2) / TIMER_HZ - 1;

        assert!(reload <= 0x00ff_ffff);
        assert!(reload > 0);
        systick.target0_conf.write(|w|w.target0_timer_unit_sel().clear_bit()); //select unit0 for
        //comp
        unsafe{systick.target0_conf.write(|w|w.target0_period().bits(160_000));} //set comp
        //value, 16MHZ/160k = 100Hz
        systick.comp0_load.write(|w|w.timer_comp0_load().set_bit()); //sync period to comp register 
        systick.target0_conf.write(|w|w.target0_period_mode().set_bit()); //enable period mode
        systick.conf.write(|w|w.target0_work_en().set_bit()); //enable comparisons
        systick.int_ena.write(|w|w.target0_int_ena().set_bit()); //enable interrupts

        const INTERRUPT_MAP_BASE: u32 = 0x600c2000; //this isn't exposed properly in the PAC,
                                                //should maybe figure out a workaround that
                                                //doesnt involve raw pointers.
                                                //Again, this is how they do it in the HAL
                                                //but i'm really not a fan.
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

        SYSTICK_TIMER_QUEUE.initialize(Systick {});
    }

    fn systick() -> SYSTIMER {
        unsafe { core::mem::transmute::<(), SYSTIMER>(()) }
    }
}

static SYSTICK_CNT: AtomicU32 = AtomicU32::new(0);
static SYSTICK_TIMER_QUEUE: TimerQueue<Systick> = TimerQueue::new();

// Forward timerqueue interface
impl Systick {
    /// Used to access the underlying timer queue
    #[doc(hidden)]
    pub fn __tq() -> &'static TimerQueue<Systick> {
        &SYSTICK_TIMER_QUEUE
    }

    /// Timeout at a specific time.
    pub async fn timeout_at<F: Future>(
        instant: <Self as Monotonic>::Instant,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        SYSTICK_TIMER_QUEUE.timeout_at(instant, future).await
    }

    /// Timeout after a specific duration.
    #[inline]
    pub async fn timeout_after<F: Future>(
        duration: <Self as Monotonic>::Duration,
        future: F,
    ) -> Result<F::Output, TimeoutError> {
        SYSTICK_TIMER_QUEUE.timeout_after(duration, future).await
    }

    /// Delay for some duration of time.
    #[inline]
    pub async fn delay(duration: <Self as Monotonic>::Duration) {
        SYSTICK_TIMER_QUEUE.delay(duration).await;
    }

    /// Delay to some specific time instant.
    #[inline]
    pub async fn delay_until(instant: <Self as Monotonic>::Instant) {
        SYSTICK_TIMER_QUEUE.delay_until(instant).await;
    }
}

impl Monotonic for Systick {
    type Instant = fugit::TimerInstantU32<TIMER_HZ>;
    type Duration = fugit::TimerDurationU32<TIMER_HZ>;
    const ZERO: Self::Instant = Self::Instant::from_ticks(0);
    fn now() -> Self::Instant {
        let peripherals = unsafe{esp32c3::Peripherals::steal()};
        peripherals.SYSTIMER.unit0_op.write(|w|w.timer_unit0_update().set_bit()); //load the timer
        let instant = unsafe{esp32c3::Peripherals::steal()}.SYSTIMER.unit0_value_lo.read().bits();
        Self::Instant::from_ticks(instant/160000)
    }

    fn set_compare(_: Self::Instant) {
        // No need to do something here, we get interrupts anyway.
    }

    fn clear_compare_flag() {
        use esp32c3::Peripherals;
        unsafe{Peripherals::steal()}.SYSTIMER.int_clr.write(|w|w.target0_int_clr().set_bit());
    }

    fn pend_interrupt() {
        extern "C" {
        fn cpu_int_31_handler();
        }
        //run the timer interrupt handler in a critical section to emulate a max priority
        //interrupt.
        //since there is no hardware support for pending a timer interrupt.
        unsafe{riscv::interrupt::disable()};
        unsafe{cpu_int_31_handler()};
        unsafe{riscv::interrupt::enable()};
    }

    fn on_interrupt() {
    }

    fn enable_timer() {}

    fn disable_timer() {}
}

#[cfg(feature = "embedded-hal-async")]
impl embedded_hal_async::delay::DelayUs for Systick {
    async fn delay_us(&mut self, us: u32) {
        Self::delay(us.micros()).await;
    }

    async fn delay_ms(&mut self, ms: u32) {
        Self::delay(ms.millis()).await;
    }
}

/// Register the Systick interrupt for the monotonic.
#[macro_export]
macro_rules! create_systick_token {
    () => {{
        #[export_name="cpu_int_31_handler"]
        #[allow(non_snake_case)]
        unsafe extern "C" fn SysTick() {
            rtic_monotonics::esp32c3::Systick::__tq().on_monotonic_interrupt();
        }

        pub struct SystickToken;

        unsafe impl $crate::InterruptToken<rtic_monotonics::esp32c3::Systick> for SystickToken {}

        SystickToken
    }};
}

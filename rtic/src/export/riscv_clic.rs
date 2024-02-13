pub use clic::peripherals::{Peripherals, CLIC}; //priority threshold control
pub use clic::interrupt::Interrupt;
pub use clic::register::mintthresh;

#[cfg(all(feature = "riscv-clic", not(feature = "riscv-clic-backend")))]
compile_error!("Building for the CLIC, but 'riscv-clic-backend not selected'");

#[inline(always)]
pub fn run<F>(priority: u8, f: F)
where
    F: FnOnce(),
{
    f();
}

/// Lock implementation using threshold and global Critical Section (CS)
///
/// # Safety
///
/// The system ceiling is raised from current to ceiling
/// by either
/// - raising the threshold to the ceiling value, or
/// - disable all interrupts in case we want to
///   mask interrupts with maximum priority
///
/// Dereferencing a raw pointer inside CS
///
/// The priority.set/priority.get can safely be outside the CS
/// as being a context local cell (not affected by preemptions).
/// It is merely used in order to omit masking in case current
/// priority is current priority >= ceiling.
#[inline(always)]
pub unsafe fn lock<T, R>(ptr: *mut T, ceiling: u8, f: impl FnOnce(&mut T) -> R) -> R {
    if ceiling == (15) {
        //turn off interrupts completely, were at max prio
        let r = critical_section::with(|_| f(&mut *ptr));
        r
    } else {
        let current = mintthresh::read();
        mintthresh::write((ceiling+1)as usize);
        let r = f(&mut *ptr);
        mintthresh::write(current);
        r
    }
}

/// Sets the given software interrupt as pending
#[inline(always)]
pub fn pend<T: Pend>(int: T) {
    unsafe{<T as Pend>::pend_int()};
}

// Sets the given software interrupt as not pending
pub fn unpend(int: Interrupt) {
    unsafe{Peripherals::steal().CLIC.unpend(int)}
}

pub fn enable(int: Interrupt, prio: u8) {
    unsafe{
        let mut p = Peripherals::steal();
        p.CLIC.set_priority(int, prio);
        p.CLIC.unmask(int);
    }
}

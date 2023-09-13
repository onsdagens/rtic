use clic::peripherals::{Peripherals, CLIC}; //priority threshold control
use clic::interrupt::Interrupts;
use clic::register::mintthresh;

#[cfg(all(feature = "riscv-clic", not(feature = "riscv-clic-backend")))]
compile_error!("Building for the esp32c3, but 'riscv-esp32c3-backend not selected'");

#[inline(always)]
pub fn run<F>(priority: u8, f: F)
where
    F: FnOnce(),
{
    if priority == 1 {
        //if priority is 1, priority thresh should be 1
        f();
        unsafe {
            mintthresh::write(1);
        }
    } else {
        //read current thresh
        let initial = unsafe {
            mintthresh::read()
        };
        f();
        //write back old thresh
        unsafe {
            mintthresh::write(initial);
        }
    }
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
        let current = unsafe {
            mintthresh::read()
        };

        unsafe {
            mintthresh::write((ceiling+1)as usize)
        } //esp32c3 lets interrupts with prio equal to threshold through so we up it by one
        let r = f(&mut *ptr);
        unsafe {
            mintthresh::write(current)
        }
        r
    }
}

/// Sets the given software interrupt as pending
#[inline(always)]
pub fn pend(int: Interrupts) {
    CLIC::pend(int);
}

// Sets the given software interrupt as not pending
pub fn unpend(int: Interrupts) {
    CLIC::unpend(int)
}

pub fn enable(int: Interrupts, prio: u8, cpu_int_id: u8) {
    unsafe{
        Peripherals::steal().CLIC.set_priority(int, prio);
        CLIC::unmask(int);
    }
}

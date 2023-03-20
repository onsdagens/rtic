pub use esp32c3::{Peripherals};
pub use riscv::{interrupt};


#[inline(always)]
pub fn run<F>(priority: u8, f: F)
where
    F: FnOnce(),
{
    if priority == 1 {
        //if priority is 1, priority thresh should be 1
        f();
        unsafe {
            (*esp32c3::INTERRUPT_CORE0::ptr()).
            cpu_int_thresh.
            write(|w|{w.cpu_int_thresh().bits(1)});
        }
    } else {
        //just a read so safe
        let initial = unsafe{
            (*esp32c3::INTERRUPT_CORE0::ptr())
            .cpu_int_thresh.read()
            .cpu_int_thresh()
            .bits()
        };

        f();
        //write back the old value, safe
        unsafe {
            (*esp32c3::INTERRUPT_CORE0::ptr()).
            cpu_int_thresh.
            write(|w|{
                w.cpu_int_thresh().
                bits(initial)
            });
        }
    }
}

/// Lock implementation using BASEPRI and global Critical Section (CS)
///
/// # Safety
///
/// The system ceiling is raised from current to ceiling
/// by either
/// - raising the BASEPRI to the ceiling value, or
/// - disable all interrupts in case we want to
///   mask interrupts with maximum priority
///
/// Dereferencing a raw pointer inside CS
///
/// The priority.set/priority.get can safely be outside the CS
/// as being a context local cell (not affected by preemptions).
/// It is merely used in order to omit masking in case current
/// priority is current priority >= ceiling.
///
/// Lock Efficiency:
/// Experiments validate (sub)-zero cost for CS implementation
/// (Sub)-zero as:
/// - Either zero OH (lock optimized out), or
/// - Amounting to an optimal assembly implementation
///   - The BASEPRI value is folded to a constant at compile time
///   - CS entry, single assembly instruction to write BASEPRI
///   - CS exit, single assembly instruction to write BASEPRI
///   - priority.set/get optimized out (their effect not)
/// - On par or better than any handwritten implementation of SRP
///
/// Limitations:
/// The current implementation reads/writes BASEPRI once
/// even in some edge cases where this may be omitted.
/// Total OH of per task is max 2 clock cycles, negligible in practice
/// but can in theory be fixed.
///
#[inline(always)]
pub unsafe fn lock<T, R>(
    ptr: *mut T,
    ceiling: u8,
    thresh_prio_bits: u8,
    f: impl FnOnce(&mut T) -> R,
) -> R {
    if ceiling == (thresh_prio_bits) {
        let r = critical_section::with(|_| f(&mut *ptr));
        r
    } else {
         let current = unsafe{(*esp32c3::INTERRUPT_CORE0::ptr()).cpu_int_thresh.read()
            .cpu_int_thresh()
            .bits()};

    
        unsafe{(*esp32c3::INTERRUPT_CORE0::ptr()).cpu_int_thresh.write(|w|w.cpu_int_thresh().bits(ceiling))}
        let r = f(&mut *ptr);
        unsafe{(*esp32c3::INTERRUPT_CORE0::ptr()).cpu_int_thresh.write(|w|w.cpu_int_thresh().bits(current))}
        r
    }
}

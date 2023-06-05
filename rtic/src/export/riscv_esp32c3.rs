//use esp32c3::INTERRUPT_CORE0; //priority threshold control
//pub use esp32c3::{Peripherals};
pub use riscv::{interrupt, register::mcause}; //low level interrupt enable/disable
pub use ral_registers::*;
pub use esp32c3_ral::Instances;
//pub use esp32c3_ral as esp32c3;
pub use esp32c3_ral::Interrupt;
use esp32c3_ral::{interrupt_core0, system};
#[cfg(all(feature = "riscv-esp32c3", not(feature = "riscv-esp32c3-backend")))]
compile_error!("Building for the esp32c3, but 'riscv-esp32c3-backend not selected'");

#[inline(always)]
pub fn run<F>(priority: u8, f: F)
where
    F: FnOnce(),
{   let ic0 = unsafe{interrupt_core0::INTERRUPT_CORE0::instance()};
    if priority == 1 {
        //if priority is 1, priority thresh should be 1
        f();
        write_reg!(interrupt_core0, ic0, CPU_INT_THRESH, 1);
    } else {
        
        let initial = read_reg!(interrupt_core0, ic0, CPU_INT_THRESH);
        f();
        //write back old thresh
        write_reg!(interrupt_core0, ic0, CPU_INT_THRESH, initial);
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
        let ic0 = unsafe{interrupt_core0::INTERRUPT_CORE0::instance()};
        let current = read_reg!(interrupt_core0, ic0, CPU_INT_THRESH);

        write_reg!(interrupt_core0, ic0, CPU_INT_THRESH, (ceiling + 1).into()); //esp32c3 lets interrupts with prio equal to threshold through so we up it by one
        let r = f(&mut *ptr);
        write_reg!(interrupt_core0, ic0, CPU_INT_THRESH, current);
        r
    }
}

/// Sets the given software interrupt as pending
#[inline(always)]
pub fn pend(int: Interrupt) {
    unsafe {
        let sys = system::SYSTEM::instance();
        match int {
            Interrupt::FROM_CPU_INTR0 => write_reg!(system, sys, CPU_INTR_FROM_CPU_0, 1),
            Interrupt::FROM_CPU_INTR1 => write_reg!(system, sys, CPU_INTR_FROM_CPU_1, 1),
            Interrupt::FROM_CPU_INTR2 => write_reg!(system, sys, CPU_INTR_FROM_CPU_2, 1),
            Interrupt::FROM_CPU_INTR3 => write_reg!(system, sys, CPU_INTR_FROM_CPU_3, 1),
            _ => panic!("Unsupported software interrupt"), //should never happen, checked at compile time
        }
    }
}
// Sets the given software interrupt as not pending
pub fn unpend(int: Interrupt) {
    unsafe {
        let sys = system::SYSTEM::instance();
        match int {
            Interrupt::FROM_CPU_INTR0 => write_reg!(system, sys, CPU_INTR_FROM_CPU_0, 0),
            Interrupt::FROM_CPU_INTR1 => write_reg!(system, sys, CPU_INTR_FROM_CPU_1, 0),
            Interrupt::FROM_CPU_INTR2 => write_reg!(system, sys, CPU_INTR_FROM_CPU_2, 0),
            Interrupt::FROM_CPU_INTR3 => write_reg!(system, sys, CPU_INTR_FROM_CPU_3, 0),
            _ => panic!("Unsupported software interrupt"),
        }
    }
}
const PRIORITY_TO_INTERRUPT: [usize; 15] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15];

pub fn enable(int: Interrupt, prio: u8, cpu_int_id: u8) {
    const INTERRUPT_MAP_BASE: u32 = 0x600c2000; //this isn't exposed properly in the PAC,
                                                //should maybe figure out a workaround that
                                                //doesnt involve raw pointers.
                                                //Again, this is how they do it in the HAL
                                                //but i'm really not a fan.
    let interrupt_number = int as isize;
    let cpu_interrupt_number = cpu_int_id as isize;
    unsafe {
        let intr_map_base = INTERRUPT_MAP_BASE as *mut u32;
        let ic0 = interrupt_core0::INTERRUPT_CORE0::instance();
        intr_map_base
            .offset(interrupt_number)
            .write_volatile(cpu_interrupt_number as u32);
        //map peripheral interrupt to CPU interrupt
        modify_reg!(interrupt_core0, ic0, CPU_INT_ENABLE,|reg|reg| 1 << cpu_interrupt_number); //enable CPU interrupt
        //this can probably be fixed by expressing the int prio registers
        //as a block in the RAL so it can be indexed into.
        let intr_prio_base = (INTERRUPT_MAP_BASE + 0x0114 + (cpu_interrupt_number*4) as u32) as *mut u32;
        intr_prio_base
            .write_volatile(prio as u32);
    }
}

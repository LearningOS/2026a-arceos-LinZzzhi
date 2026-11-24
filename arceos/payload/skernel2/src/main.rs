#![no_std]
#![no_main]

use core::panic::PanicInfo;

#[no_mangle]
unsafe extern "C" fn _start() -> ! {
    core::arch::asm!(
        // Reset type / reason expected by the hypervisor's VM-exit handler.
        "li a0, 0x6688",
        "li a1, 0x1234",
        // Legacy SBI shutdown extension.
        "li a7, 8",
        "ecall",
        options(noreturn)
    )
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

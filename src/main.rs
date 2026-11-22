#![no_std]
#![no_main]

mod gfx;
mod fs;
mod gui;
mod input;
mod term;

use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    let mut g = gfx::Gfx::new();
    input::init();
    let mut inp = input::Input::new();
    let mut desk = gui::Desktop::new();
    loop {
        while let Some(e) = inp.poll() {
            desk.event(e);
        }
        desk.draw(&mut g);
        g.present();
    }
}

#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    gfx::fill_screen(gfx::ACCENT); // ponytail: no text in graphics mode yet, solid orange = panic
    loop {
        unsafe { core::arch::asm!("hlt") };
    }
}

#![no_std]
#![no_main]

use esp8266_hal::prelude::*;
use esp8266_hal::target::Peripherals;
use panic_halt as _;

#[entry]
fn main() -> ! {
    let _dp = Peripherals::take().unwrap();

    // Initialize WiFi
    let wifi = esp8266_wifi::EspWifi::init();
    match wifi {
        Ok(_w) => {
            // Success!
        }
        Err(_e) => {
            // Failed
        }
    }

    loop {
        continue;
    }
}

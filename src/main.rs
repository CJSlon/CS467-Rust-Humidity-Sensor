
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_time::Timer;
use panic_halt as _;

async fn illuminate_led(led: &mut Output<'_>) {
    //! Function to turn on LED pin
    //! Args:
    //!    led: Mutable reference to an Output pin
    //!Returns:
    //!    None

    led.set_high();
}

async fn dim_led(led: &mut Output<'_>) {
    //! Function to turn off LED pin
    //! Args:
    //!    led: Mutable reference to an Output pin
    //!Returns:
    //!    None
    
    led.set_low();
}

async fn boot_led_sequence(leds: &mut [Output<'_>]) {
    //! Function to run boot sequence on all LEDs to ack startup/function
    //! Args:
    //!   leds: Mutable ref to array of leds
    //! Returns:
    //!  None
    
    // Cycle LEDs forward and backwards
    for i in 0..leds.len() {
        illuminate_led(&mut leds[i]).await;
        Timer::after_millis(100).await;
        dim_led(&mut leds[i]).await;
    }

    for i in (0..leds.len()).rev() {
        illuminate_led(&mut leds[i]).await;
        Timer::after_millis(100).await;
        dim_led(&mut leds[i]).await;
    }
}


#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default()); // initialize peripheral enum
    
    // Array of LED pin objects
    let mut leds = [
        Output::new(p.PIN_15, Level::Low), // Very low RH LED
        Output::new(p.PIN_14, Level::Low),
        Output::new(p.PIN_13, Level::Low),
        Output::new(p.PIN_12, Level::Low),
        Output::new(p.PIN_11, Level::Low),
        Output::new(p.PIN_10, Level::Low), // Very high RH LED
    ];

    loop {
        boot_led_sequence(&mut leds).await;
    }
}


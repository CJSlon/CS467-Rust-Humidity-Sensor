
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};  //import GPIO driver
use embassy_rp::i2c::{I2c, Async, Config, InterruptHandler, AbortReason};   // import I2C driver
use embassy_rp::bind_interrupts;
use embassy_rp::peripherals::I2C0;
use embassy_time::Timer;
use panic_halt as _;

// bind I2C0 interrupt to I2C driver
bind_interrupts!(struct Interupts { I2C0_IRQ => InterruptHandler<I2C0>; });

async fn illuminate_led(led: &mut Output<'_>) {
    //! Function to turn on LED pin
    //! Args:
    //!    led: Mutable reference to an Output pin
    //! Returns:
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
    for i in 0..leds.len()-1 {
        illuminate_led(&mut leds[i]).await;
        Timer::after_millis(100).await;
        dim_led(&mut leds[i]).await;
    }

    for i in (0..leds.len()-1).rev() {
        illuminate_led(&mut leds[i]).await;
        Timer::after_millis(100).await;
        dim_led(&mut leds[i]).await;
    }
}

async fn boot_error_led_sequence(leds: &mut [Output<'_>]){
    //! function to display an error sequence on LEDs (three quick flashes) 
    //! Args:
    //!     leds: Mutable ref to array of leds
    //! Returns:
    //!     None

    for _ in 0..3 {
        for i in 0..leds.len() {
            illuminate_led(&mut leds[i]).await;
        }
        Timer::after_millis(100).await;
        for i in 0..leds.len() {
            dim_led(&mut leds[i]).await;
        }        Timer::after_millis(100).await;
        Timer::after_millis(100).await;
    }
}

async fn dht20_init(dht20_i2c: &mut I2c<'static, I2C0, Async>, dht20_address: u8, dht20_status: u8) -> Result<(), embassy_rp::i2c::Error> {
    //! Function to initialize DHT2 sensor
    //! ARGS:
    //!    dht20_i2c : mutable ref to I2C object for the DHT20 sensor with object parameters:
    //!         'static : lifetime specifier
    //!         I2C0 : periperhal instance--change based on pin and which I2C is used
    //!         Async : Async mode for I2C communications (Change to blocking if needed)
    //!    dht20_address : u8 : I2C address of the sensor 
    //!    dht20_status : u8 : status register address of sensor     
    //! Returns:
    //!    Result: okay : if sensor is init correctly
    //!    Restul: Error : if I2C transmission fails or status bits are incorrect
    
    let mut status_buffer = [0x00]; // buffer to hold status word rad from sensor
    Timer::after_millis(500).await;   // warmup delay based on DHT20 data sheet

    dht20_i2c.write_read_async(dht20_address, [dht20_status], &mut status_buffer).await?;

    // bitwise AND to check for correct status bits
    if status_buffer[0] & 0x18 == 0x18 {
        Ok(())
    } else {
        Err(embassy_rp::i2c::Error::Abort(AbortReason::Other(0)))
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
        Output::new(p.PIN_25, Level::Low), // Onboard LED
    ];

    // I2C object for sensor
    let mut dht20_i2c = I2c::new_async(p.I2C0, p.PIN_21, p.PIN_20, Interupts, Config::default());

    // dht20 register addresses and commands
    const DHT20_ADDRESS: u8 = 0x38; // I2C Address for DHT20 per data sheet
    const DHT20_STATUS: u8 = 0x71; // status register address for DHT20 per data sheet

    // Initialize DHT20 sensor and run boot/error sequence based on result
    match dht20_init(&mut dht20_i2c, DHT20_ADDRESS, DHT20_STATUS).await {
        Ok(()) => {
            // Initialization is successful run boot sequence on LEDs
            loop { // when integrating with read logic run boot seq once and then break to main logic
                boot_led_sequence(&mut leds).await;
            }
        }
        Err(_e) => {
            // If init fails run error pattern on LEDs
            loop {
                boot_error_led_sequence(&mut leds).await;
            }
        }
    }
}


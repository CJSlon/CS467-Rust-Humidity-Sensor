#![no_std]
#![no_main]

use panic_halt as _;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::{
    bind_interrupts,
    i2c::{self, Config as I2cConfig, InterruptHandler},
    peripherals::I2C0,
};
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c as AsyncI2c;

const DHT20_ADDR: u8 = 0x38;
const SENSOR_TRIGGER_CMD: [u8; 3] = [0xAC, 0x33, 0x00];
const SENSOR_READ_WAIT_MS: u64 = 80;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

async fn blink_led(mut led: Output<'static>) {
    //! Function to blink onboard LED to signal successful boot
    //! Args:
    //!    led: Mutable Output pin (owned)
    //! Returns:
    //!    None

    for _ in 0..10 {
        led.set_high();
        Timer::after_millis(100).await;
        led.set_low();
        Timer::after_millis(100).await;
    }
    led.set_high();
}

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

async fn blink_error_led(led: &mut Output<'_>) {
    //! Function to continuously blink an LED to signal an error state
    //! Args:
    //!    led: Mutable reference to an Output pin
    //!Returns:
    //!    None

    loop {
        illuminate_led(led).await;
        Timer::after_millis(500).await;
        dim_led(led).await;
        Timer::after_millis(500).await;
    }
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

async fn get_humidity_sensor_data<I: AsyncI2c>(dht_i2c: &mut I) -> (bool, bool, bool, [u8; 6]) {
    //! Function to trigger and read raw data from the DHT20 sensor over I2C
    //! Args:
    //!    dht_i2c: Mutable reference to an async I2C device
    //! Returns:
    //!    Tuple of (busy, read_err, write_err, data)

    let mut busy = true; // Sensor is still processing measurement
    let mut read_err = false; // Error flag for I2C read
    let mut write_err = false; // Error flag for I2C write
    let mut data = [0u8; 6]; // Buffer to hold raw sensor data

    if dht_i2c.write(DHT20_ADDR, &SENSOR_TRIGGER_CMD).await.is_ok() {
        Timer::after_millis(SENSOR_READ_WAIT_MS).await;

        // Make multiple read attempts in case sensor is still busy, but break early if we get a successful read
        for _ in 0..10 {
            match dht_i2c.read(DHT20_ADDR, &mut data).await {
                Ok(()) => {
                    // 0 in the MSB of the first byte indicates measurement is ready
                    if (data[0] & 0x80) == 0 {
                        busy = false;
                        break;
                    }
                }
                Err(_) => {
                    read_err = true;
                    break;
                }
            }
            Timer::after_millis(10).await;
        }
    } else {
        write_err = true;
    }
    (busy, read_err, write_err, data)
}

fn process_sensor_data(data: [u8; 6]) -> f32 {
    //! Function to convert raw DHT20 sensor bytes into a relative humidity percentage
    //! Args:
    //!    data: Array of 6 raw bytes from the sensor
    //! Returns:
    //!    Humidity as a f32 percentage

    let raw_humidity: u32 =
        ((data[1] as u32) << 12) | ((data[2] as u32) << 4) | ((data[3] as u32) >> 4);
    let humidity = (raw_humidity as f32) * 100.0 / 1048576.0;
    humidity
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    // Array of LED pin objects
    let mut leds = [
        Output::new(p.PIN_15, Level::Low), // Very low RH LED
        Output::new(p.PIN_14, Level::Low),
        Output::new(p.PIN_13, Level::Low),
        Output::new(p.PIN_12, Level::Low),
        Output::new(p.PIN_11, Level::Low),
        Output::new(p.PIN_10, Level::Low), // Very high RH LED
    ];

    // testing onboard LED on pin 25 to verify async functionality before setting up I2C
    let led = Output::new(p.PIN_25, Level::Low);
    blink_led(led).await;

    // Set up async I2C bus
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 100_000;

    let mut dht_i2c = i2c::I2c::new_async(p.I2C0, p.PIN_17, p.PIN_16, Irqs, i2c_config);

    // Main loop
    loop {
        boot_led_sequence(&mut leds).await;
        let (busy, read_err, write_err, data) = get_humidity_sensor_data(&mut dht_i2c).await;

        if write_err {
            blink_error_led(&mut leds[0]).await;
        } else if read_err {
            blink_error_led(&mut leds[1]).await;
        } else if busy {
            blink_error_led(&mut leds[2]).await;
        } else {
            let humidity = process_sensor_data(data);
            if humidity < 30.0 {
                illuminate_led(&mut leds[3]).await;
            } else if humidity < 60.0 {
                illuminate_led(&mut leds[4]).await;
            } else {
                illuminate_led(&mut leds[5]).await;
            }
        }

        Timer::after_millis(1000).await;
    }
}

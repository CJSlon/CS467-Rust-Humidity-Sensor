// DHT20 Data Sheet
// https://cdn-shop.adafruit.com/product-files/5183/5193_DHT20.pdf
// https://pip-assets.raspberrypi.com/categories/610-raspberry-pi-pico/documents/RP-008307-DS-1-pico-datasheet.pdf?disposition=inline
// https://pico.implrust.com/lcd-display/hello-rust.html

#![no_std]
#![no_main]

use panic_halt as _;

use core::{cell::RefCell, fmt::Write};

use embassy_embedded_hal::shared_bus::blocking::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::i2c::{self, Config as I2cConfig};
use embassy_sync::blocking_mutex::{raw::NoopRawMutex, Mutex};
use embassy_time::{Delay, Timer};
use embedded_hal::blocking::i2c::{Read as I2cRead, Write as I2cWrite};
use hd44780_driver::HD44780;
use heapless::String;

const DHT20_ADDR: u8 = 0x38; // I2C address for DHT20 sensor
const LCD_ADDR_DEFAULT: u8 = 0x27; // I2C address for LCD

// Pad a line to 16 characters with spaces for LCD display
fn pad_line(line: &mut String<16>) {
    while line.len() < 16 {
        let _ = line.push(' ');
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_rp::init(Default::default());

    let mut led = Output::new(p.PIN_25, Level::Low); // On-board LED
    let mut delay = Delay;

    // Blink rapidly to show program started
    for _ in 0..10 {
        led.set_high();
        Timer::after_millis(100).await;
        led.set_low();
        Timer::after_millis(100).await;
    }

    led.set_high();

    // Set up I2C bus
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 100_000;

    let i2c = i2c::I2c::new_blocking(p.I2C0, p.PIN_17, p.PIN_16, i2c_config);
    let i2c_bus: Mutex<NoopRawMutex, RefCell<_>> = Mutex::new(RefCell::new(i2c));

    let lcd_i2c = I2cDevice::new(&i2c_bus); // Separate device for LCD
    let mut dht_i2c = I2cDevice::new(&i2c_bus); // Separate device for DHT20

    // Initialize and test LCD. Just used to verify humidity readings.
    let mut lcd = HD44780::new_i2c(lcd_i2c, LCD_ADDR_DEFAULT, &mut delay).unwrap();
    let _ = lcd.reset(&mut delay);
    let _ = lcd.clear(&mut delay);
    let _ = lcd.write_str("DHT20 init...", &mut delay);
    led.set_low(); // Success

    // Main loop to read DHT20 and display on LCD
    loop {
        let mut line2: String<16> = String::new();
        let mut data = [0u8; 6];

        // Trigger measurement
        if dht_i2c.write(DHT20_ADDR, &[0xAC, 0x33, 0x00]).is_ok() {
            Timer::after_millis(80).await; // Wait for measurement based on data sheet

            let mut busy = true;
            let mut read_err = false;
            for _ in 0..5 {
                match dht_i2c.read(DHT20_ADDR, &mut data) {
                    //check bit 7 as specified in datasheet
                    Ok(()) => {
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

            if read_err {
                let _ = write!(&mut line2, "DHT20 I2C Err");
            } else if busy {
                let _ = write!(&mut line2, "DHT20 Busy");
            } else {
                let raw_humidity: u32 =
                    ((data[1] as u32) << 12) | ((data[2] as u32) << 4) | ((data[3] as u32) >> 4); // read 20 bits for humidity
                let raw_temp: u32 =
                    (((data[3] as u32) & 0x0F) << 16) | ((data[4] as u32) << 8) | (data[5] as u32); // read 20 bits for temperature

                let humidity = (raw_humidity as f32) * 100.0 / 1048576.0; // calculate humidity
                let temperature = (raw_temp as f32) * 200.0 / 1048576.0 - 50.0; // calculate temperature

                let _ = write!(&mut line2, "H:{:>5.1}% T:{:>5.1}", humidity, temperature);
                // Format output
            }
        } else {
            let _ = write!(&mut line2, "DHT20 No Ack"); // No ACK from sensor
        }

        pad_line(&mut line2);
        let _ = lcd.set_cursor_pos(0x40, &mut delay);
        let _ = lcd.write_str(&line2, &mut delay); // Display on LCD to verify readings

        Timer::after_millis(1000).await;
    }
}

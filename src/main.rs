// DHT20 with minimal async LCD support

#![no_std]
#![no_main]

use panic_halt as _;

use core::fmt::Write;

use embassy_embedded_hal::shared_bus::asynch::i2c::I2cDevice;
use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output};
use embassy_rp::{
    bind_interrupts,
    i2c::{self, Config as I2cConfig, InterruptHandler},
    peripherals::I2C0,
};
use embassy_sync::blocking_mutex::raw::NoopRawMutex;
use embassy_sync::mutex::Mutex;
use embassy_time::Timer;
use embedded_hal_async::i2c::I2c as AsyncI2c;
use heapless::String;

const DHT20_ADDR: u8 = 0x38;
const LCD_ADDR: u8 = 0x27;
const SENSOR_TRIGGER_CMD: [u8; 3] = [0xAC, 0x33, 0x00];
const SENSOR_READ_WAIT_MS: u64 = 80;

bind_interrupts!(struct Irqs {
    I2C0_IRQ => InterruptHandler<I2C0>;
});

// Minimal async LCD driver for PCF8574 I2C backpack
struct SimpleLcd<'a, I: AsyncI2c> {
    i2c: I2cDevice<'a, NoopRawMutex, I>,
    addr: u8,
    backlight: u8,
}

impl<'a, I: AsyncI2c> SimpleLcd<'a, I> {
    fn new(i2c: I2cDevice<'a, NoopRawMutex, I>, addr: u8) -> Self {
        Self {
            i2c,
            addr,
            backlight: 0x08,
        }
    }

    async fn write_nibble(&mut self, data: u8) {
        let _ = self.i2c.write(self.addr, &[data | self.backlight]).await;
        Timer::after_micros(1).await;
        let _ = self
            .i2c
            .write(self.addr, &[data | 0x04 | self.backlight])
            .await;
        Timer::after_micros(1).await;
        let _ = self.i2c.write(self.addr, &[data | self.backlight]).await;
        Timer::after_micros(50).await;
    }

    async fn send(&mut self, data: u8, mode: u8) {
        let high = (data & 0xF0) | mode;
        let low = ((data << 4) & 0xF0) | mode;
        self.write_nibble(high).await;
        self.write_nibble(low).await;
    }

    async fn init(&mut self) {
        Timer::after_millis(50).await;
        self.write_nibble(0x30).await;
        Timer::after_millis(5).await;
        self.write_nibble(0x30).await;
        Timer::after_micros(100).await;
        self.write_nibble(0x30).await;
        self.write_nibble(0x20).await;

        self.send(0x28, 0).await; // 4-bit, 2 line
        self.send(0x0C, 0).await; // Display on
        self.send(0x06, 0).await; // Entry mode
        self.send(0x01, 0).await; // Clear
        Timer::after_millis(2).await;
    }

    async fn clear(&mut self) {
        self.send(0x01, 0).await;
        Timer::after_millis(2).await;
    }

    async fn set_cursor(&mut self, row: u8, col: u8) {
        let pos = if row == 0 { 0x80 | col } else { 0xC0 | col };
        self.send(pos, 0).await;
    }

    async fn print(&mut self, text: &str) {
        for byte in text.bytes() {
            self.send(byte, 0x01).await;
        }
    }
}

fn pad_line(line: &mut String<16>) {
    while line.len() < 16 {
        let _ = line.push(' ');
    }
}

async fn blink_led(mut led: Output<'static>) {
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
    let mut busy = true;
    let mut read_err = false;
    let mut write_err = false;
    let mut data = [0u8; 6];

    if dht_i2c.write(DHT20_ADDR, &SENSOR_TRIGGER_CMD).await.is_ok() {
        Timer::after_millis(SENSOR_READ_WAIT_MS).await;

        for _ in 0..10 {
            match dht_i2c.read(DHT20_ADDR, &mut data).await {
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
    } else {
        write_err = true;
    }
    (busy, read_err, write_err, data)
}

fn process_sensor_data(data: [u8; 6]) -> (f32, f32) {
    let raw_humidity: u32 =
        ((data[1] as u32) << 12) | ((data[2] as u32) << 4) | ((data[3] as u32) >> 4);
    let raw_temp: u32 =
        (((data[3] as u32) & 0x0F) << 16) | ((data[4] as u32) << 8) | (data[5] as u32);

    let humidity = (raw_humidity as f32) * 100.0 / 1048576.0;
    let temperature = (raw_temp as f32) * 200.0 / 1048576.0 - 50.0;

    (humidity, temperature)
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

    let led = Output::new(p.PIN_25, Level::Low);
    blink_led(led).await;

    // Set up async I2C bus
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 100_000;

    let i2c = i2c::I2c::new_async(p.I2C0, p.PIN_17, p.PIN_16, Irqs, i2c_config);
    let i2c_bus: Mutex<NoopRawMutex, _> = Mutex::new(i2c);

    let mut dht_i2c = I2cDevice::new(&i2c_bus);
    let lcd_i2c = I2cDevice::new(&i2c_bus);

    let mut lcd = SimpleLcd::new(lcd_i2c, LCD_ADDR);
    lcd.init().await;
    lcd.print("DHT20 init...").await;

    // Main loop
    loop {
        boot_led_sequence(&mut leds).await;
        let mut line2: String<16> = String::new();

        let (busy, read_err, write_err, data) = get_humidity_sensor_data(&mut dht_i2c).await;

        if write_err {
            let _ = write!(&mut line2, "DHT20 No Ack");
        } else if read_err {
            let _ = write!(&mut line2, "DHT20 I2C Err");
        } else if busy {
            let _ = write!(&mut line2, "DHT20 Busy");
        } else {
            let (humidity, temperature) = process_sensor_data(data);
            let _ = write!(&mut line2, "H:{:>5.1}% T:{:>5.1}", humidity, temperature);
        }

        pad_line(&mut line2);
        lcd.set_cursor(1, 0).await;
        lcd.print(&line2).await;

        Timer::after_millis(1000).await;
    }
}

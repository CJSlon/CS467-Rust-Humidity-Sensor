#![no_std]
#![no_main]

use panic_halt as _;

use embassy_executor::Spawner;
use embassy_rp::gpio::{Level, Output}; //import GPIO driver
use embassy_rp::{
    bind_interrupts,
    i2c::{self, Config as I2cConfig, InterruptHandler, AbortReason, Async, I2c},
    peripherals::I2C0,
};

use embassy_time::Timer;
use embedded_hal_async::i2c::I2c as AsyncI2c;

const DHT20_ADDRESS: u8 = 0x38; // I2C Address for DHT20 per data sheet
const SENSOR_TRIGGER_CMD: [u8; 3] = [0xAC, 0x33, 0x00];
const SENSOR_READ_WAIT_MS: u64 = 80;
const DHT20_STATUS: u8 = 0x71; // status register address for DHT20 per data sheet
const IIR_ALPHA: f32 = 0.25; // the IIR filter value parameter
const PATTERN_RENDER_THRESHOLDS: [f32; 6] = [0.0, 20.0, 40.0, 50.0, 60.0, 70.0];

bind_interrupts!(struct Irqs { I2C0_IRQ => InterruptHandler<I2C0>; });

async fn blink_led(led: &mut Output<'static>) {
    //! Function to blink onboard LED
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

async fn read_ack_led(led: &mut Output<'_>) {
    //! Function blinks LED once to acknowledge a successful sensor read
    //! Args:
    //!    led: Mutable reference to an Output pin
    //! Returns:
    //!    None

    illuminate_led(led).await;
    Timer::after_millis(200).await;
    dim_led(led).await;
    Timer::after_millis(200).await;
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

    if dht_i2c.write(DHT20_ADDRESS, &SENSOR_TRIGGER_CMD).await.is_ok() {
        Timer::after_millis(SENSOR_READ_WAIT_MS).await;

        // Make multiple read attempts in case sensor is still busy, but break early if we get a successful read
        for _ in 0..10 {
            match dht_i2c.read(DHT20_ADDRESS, &mut data).await {
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

fn filter_iir(value: f32, prev_filter_value: f32, alpha: f32) -> f32 {
    //! Function to retrieve the next IIR-filtered signal value
    //! Args:
    //!    value: the current signal value 
    //!    prev_filter_value: the last computed filter value 
    //!           (use the current value as the previous value for the first measurement)
    //!    alpha: the IIR filter constant (filtered value = (1 - alpha) * prev filterd value + alpha * value )
    //! Returns:
    //!    An updated filtered signal value

    (1.0 - alpha) * prev_filter_value + alpha * value
}

fn render_pattern(value: f32, thresholds: [f32; 6]) -> [bool; 6] {
    //! Function to render a value as a discrete pattern.
    //! Args:
    //!    value: the value to render 
    //!    thresholds: the pattern cutoff values
    //! Returns:
    //!    The pattern as a list of boolean states.

    let mut pattern = [false; 6]; 

    for (i, threshold) in thresholds.iter().enumerate() {
        if value >= *threshold { pattern[i] = true; }
    }

    pattern
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
        Output::new(p.PIN_25, Level::Low), // Onboard LED
    ];

    // Set up async I2C bus
    let mut i2c_config = I2cConfig::default();
    i2c_config.frequency = 100_000;

    // I2C object for sensor
    let mut dht20_i2c = I2c::new_async(p.I2C0, p.PIN_21, p.PIN_20, Irqs, i2c_config);

    // Initialize DHT20 sensor and run boot/error sequence based on result
    match dht20_init(&mut dht20_i2c, DHT20_ADDRESS, DHT20_STATUS).await {
        Ok(()) => {
            // Initialization is successful run boot sequence on LEDs
            boot_led_sequence(&mut leds).await;

            //Set previous filter to 0 for first measurement: outside loop so it persists
            let mut prev_filtered_humidity = 0.0;
            let mut first_measurement = true;

            loop {
                // Main operation loop
                // Read the humidity, filter it, and render the LED output
                let (busy, read_err, write_err, data) = get_humidity_sensor_data(&mut dht20_i2c).await;

                if write_err {
                    blink_error_led(&mut leds[0]).await;
                } else if read_err {
                    blink_error_led(&mut leds[1]).await;
                } else if busy {
                    blink_error_led(&mut leds[2]).await;
                } else {
                    read_ack_led(&mut leds[6]).await; // Blink onboard LED to ack successful read
                    
                    let mut humidity = process_sensor_data(data);
                    // Get the current humidity and filter it
                    humidity = process_sensor_data(data);
                        
                        
                    if first_measurement { 
                        prev_filtered_humidity = humidity;
                        first_measurement = false;
                    }

                    let filtered_humidity = filter_iir(humidity, prev_filtered_humidity, IIR_ALPHA);
                    prev_filtered_humidity = filtered_humidity;

                    // Indicate a new measurement was processed (for debuging)
                    //boot_led_sequence(&mut leds).await;

                    // Render the LED pattern from the filtered humidity measurement
                    let pattern = render_pattern(filtered_humidity, PATTERN_RENDER_THRESHOLDS);

                    // Display the rendered pattern
                    for (i, state) in pattern.iter().enumerate() {
                        match *state { 
                            true => illuminate_led(&mut leds[i]).await,
                            false => dim_led(&mut leds[i]).await,
                        }
                    }

                        // Wait before taking the next measurement
                    Timer::after_millis(2000).await;
                    
                }
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

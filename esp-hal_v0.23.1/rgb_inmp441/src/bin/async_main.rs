#![no_std]
#![no_main]

use core::mem::MaybeUninit;
use defmt::info;
use defmt_rtt as _;
use embassy_sync::{signal::Signal, blocking_mutex::raw::CriticalSectionRawMutex};
use embassy_executor::Spawner;
use embassy_time::{Duration, Ticker};
use esp_backtrace as _;
use esp_hal::{
    gpio::AnyPin,
    dma_buffers,
    i2s::master::{I2s, I2sRx, Standard, DataFormat},
    interrupt::{software::SoftwareInterruptControl, Priority},
    peripherals::RMT,
    rmt::Rmt,
    time::RateExtU32,
    timer::{timg::TimerGroup, AnyTimer},
    Async,
};
use esp_hal_embassy::InterruptExecutor;
use static_cell::StaticCell;
use esp_hal_smartled::{smartLedBuffer, SmartLedsAdapter};
use smart_leds::{
    brightness, gamma, hsv::{hsv2rgb, Hsv}, SmartLedsWrite
};

const FFT_SIZE: usize = 256; 
const MOVING_AVERAGE_WINDOW: usize = 32; 
const SOUND_THRESHOLD: i32 = 50;    

fn init_heap() {
    const HEAP_SIZE: usize = 3 * 1024;
    static mut HEAP: MaybeUninit<[u8; HEAP_SIZE]> = MaybeUninit::uninit();

    unsafe {
        esp_alloc::HEAP.add_region(esp_alloc::HeapRegion::new(
            HEAP.as_mut_ptr() as *mut u8,
            HEAP_SIZE,
            esp_alloc::MemoryCapability::Internal.into(),
        ));
    }
}


#[embassy_executor::task]
async fn high_prio() {
    info!("Starting high_prio()");
    let mut ticker = Ticker::every(Duration::from_secs(1));
    loop {
        info!("High priority ticks");
        ticker.next().await;
    }
}

#[embassy_executor::task]
async fn microphone_reader(
    i2s_rx: I2sRx<'static, Async>,
    buffer: &'static mut [u8],
    signal: &'static Signal<CriticalSectionRawMutex, [i16; FFT_SIZE]>,
) {
    info!("Starting microphone_reader task");
    
    const BYTES_PER_SAMPLE: usize = 2;
    
    let mut data = [0u8; 4096 * 2];
    let mut transaction = i2s_rx.read_dma_circular_async(buffer).unwrap();
    
    loop {
        let _avail = transaction.available().await.unwrap();
        let count = transaction.pop(&mut data).await.unwrap();
        
        if count >= FFT_SIZE * BYTES_PER_SAMPLE {
            let mut samples: [i16; FFT_SIZE] = [0i16; FFT_SIZE];
            for (i, chunk) in data.chunks_exact(4).enumerate().take(FFT_SIZE) {
                samples[i] = i16::from_le_bytes([chunk[0], chunk[1]]);
            }
            
            signal.signal(samples);
        }
    }
}

fn moving_average(samples: &[i16; FFT_SIZE]) -> [i16; FFT_SIZE] {
    let mut output = [0i16; FFT_SIZE];
    let half_window = (MOVING_AVERAGE_WINDOW / 2) as isize;

    for i in 0..FFT_SIZE {
        let mut sum: i32 = 0;
        let mut count = 0;

        for j in (i as isize - half_window)..=(i as isize + half_window) {
            if j >= 0 && j < FFT_SIZE as isize {
                sum += samples[j as usize] as i32;
                count += 1;
            }
        }
        //Properly handle cases with 0, and cast
        output[i] = if count > 0 { (sum / count as i32) as i16} else { 0 };
    }
    output
}

fn calculate_average_amplitude(samples: &[i16; FFT_SIZE]) -> i32 {
    let mut sum: i32 = 0;
    for &sample in samples.iter() {
        sum += sample.abs() as i32; // .abs() for absolute value!
    }
    sum / FFT_SIZE as i32 // Avoid integer division issues.
}


#[embassy_executor::task]
async fn audio_processor(
    signal: &'static Signal<CriticalSectionRawMutex, [i16; FFT_SIZE]>,
    led: AnyPin, 
    peripheral_rmt: RMT
) {
    info!("Starting audio_processor task");

    info!("Starting high_prio()");
    let rmt = Rmt::new(peripheral_rmt, 80.MHz()).unwrap();

    let rmt_buffer = smartLedBuffer!(1);
    let mut led = SmartLedsAdapter::new(rmt.channel0, led, rmt_buffer);

    let mut color = Hsv {
        hue: 0,
        sat: 0,
        val: 0,
    };
    let mut data;
    loop {
        let samples = signal.wait().await;

        // Apply the moving average
        let smoothed_samples = moving_average(&samples);

        // Calculate the average amplitude of the *smoothed* samples.
        let average_amplitude = calculate_average_amplitude(&smoothed_samples);

        // Now apply the threshold to the single average amplitude value.
        if average_amplitude > SOUND_THRESHOLD {
            color.val = 255; // Sound detected!
            info!("ON");
        } else {
            color.val = 0;   // No sound
            info!("OFF");
        }

        data = [hsv2rgb(color)];
        led.write(brightness(gamma(data.iter().cloned()), 150))
            .unwrap();
    }
}



#[esp_hal_embassy::main]
async fn main(low_prio_spawner: Spawner) {
    info!("Init!");

    init_heap();

    let peripherals = esp_hal::init(esp_hal::Config::default());

    let sw_ints = SoftwareInterruptControl::new(peripherals.SW_INTERRUPT);

    let timg0 = TimerGroup::new(peripherals.TIMG0);
    let timer0: AnyTimer = timg0.timer0.into();

    let timg1 = TimerGroup::new(peripherals.TIMG1);
    let timer1: AnyTimer = timg1.timer0.into();

    esp_hal_embassy::init([timer0, timer1]);

    let dma_channel = peripherals.DMA_CH0;

    let (rx_buffer, rx_descriptors, _, tx_descriptors) = dma_buffers!(4096 * 3, 0);

    let mclk = peripherals.GPIO0;
    let bclk = peripherals.GPIO2;
    let ws = peripherals.GPIO4;
    let din = peripherals.GPIO5;

    let i2s = I2s::new(
        peripherals.I2S0,
        Standard::Philips,
        DataFormat::Data16Channel16,
        44100.Hz(),
        dma_channel,
        rx_descriptors,
        tx_descriptors,
    )
    .with_mclk(mclk)
    .into_async();

    let i2s_rx = i2s
    .i2s_rx
    .with_bclk(bclk)
    .with_ws(ws)
    .with_din(din)
    .build();

    let rgb_48 = peripherals.GPIO48;
    let rmt_pref = peripherals.RMT;

    static SAMPLES_SIGNAL: StaticCell<Signal<CriticalSectionRawMutex, [i16; FFT_SIZE]>> = StaticCell::new();
    let samples_signal = &*SAMPLES_SIGNAL.init(Signal::new(), );

    static HIGH_PRIO_EXECUTOR: StaticCell<InterruptExecutor<2>> = StaticCell::new();
    let high_prio_executor = InterruptExecutor::new(sw_ints.software_interrupt2);
    let executor = HIGH_PRIO_EXECUTOR.init(high_prio_executor);

    let high_prio_spawner = executor.start(Priority::Priority3);
    low_prio_spawner.must_spawn(audio_processor(samples_signal, rgb_48.into(), rmt_pref));


    high_prio_spawner.must_spawn(microphone_reader(i2s_rx, rx_buffer, samples_signal));
}

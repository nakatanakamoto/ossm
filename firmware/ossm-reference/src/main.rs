#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

mod current_sensor;
mod step_output;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::signal::Signal;
use embassy_time::Delay;
use embassy_time::{Duration, Ticker};
use esp_hal::{Config, init};
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    gpio::{Level, Output, OutputConfig},
    interrupt::{Priority, software::SoftwareInterruptControl},
    rmt::{Rmt, TxChannelConfig, TxChannelCreator},
    system::Stack,
    time::Rate,
    timer::timg::TimerGroup,
};
use esp_radio::ble::controller::BleConnector;
use esp_rtos::embassy::InterruptExecutor;
use log::info;
use m57aim_motor::{Motor57AIM, Motor57AIMConfig};
use ossm::{MechanicalConfig, MotionController, MotionLimits, Ossm, StepDirConfig, StepDirMotor};

use current_sensor::AdcCurrentSensor;
use step_output::{RMT_CLK_DIVIDER, RmtStepOutput};
use stepdir_board::{HomingConfig, StepDirBoard};

use pattern_engine::{AnyPattern, PatternEngine};
use static_cell::StaticCell;

use {esp_backtrace as _, esp_println as _};

extern crate alloc;

esp_bootloader_esp_idf::esp_app_desc!();

const UPDATE_INTERVAL_SECS: f64 = 0.01;

macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: StaticCell<$t> = StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

type ConcreteMotor =
    Motor57AIM<StepDirMotor<RmtStepOutput, Output<'static>, Output<'static>>, Delay>;
type ConcreteBoard = StepDirBoard<ConcreteMotor, AdcCurrentSensor, Delay>;

static OSSM: Ossm = Ossm::new();
static PATTERNS: PatternEngine = PatternEngine::new(&OSSM);

static EXECUTOR_CORE_1: StaticCell<InterruptExecutor<2>> = StaticCell::new();
// ESP32 DRAM is tighter than ESP32-S3. 16KB stack is needed for ruckig's
// trajectory calculator (heavy float math). 64KB heap is needed for the BLE
// radio stack's internal allocations. ESP-NOW was dropped to fit within
// the ESP32's memory constraints.
static APP_CORE_STACK: StaticCell<Stack<32768>> = StaticCell::new();
static MOTION_READY: Signal<CriticalSectionRawMutex, bool> = Signal::new();

#[embassy_executor::task]
async fn motion_task(mut controller: MotionController<'static, ConcreteBoard>) {
    let interval_us = (UPDATE_INTERVAL_SECS * 1_000_000.0) as u64;
    let mut ticker = Ticker::every(Duration::from_micros(interval_us));

    loop {
        if let Err(e) = controller.update().await {
            log::error!("Motion controller fault: {:?}", e);
        }
        ticker.next().await;
    }
}

#[esp_rtos::main]
async fn main(spawner: Spawner) {
    ossm::logging::init(log::LevelFilter::Info, |line| {
        esp_println::println!("{}", line);
    });

    ossm::build_info!();

    let p = init(Config::default());

    esp_alloc::heap_allocator!(size: 64 * 1024);

    let timg0 = TimerGroup::new(p.TIMG0);
    esp_rtos::start(timg0.timer0);

    let rmt = Rmt::new(p.RMT, Rate::from_mhz(80)).expect("Failed to initialize RMT");
    let tx_config = TxChannelConfig::default().with_clk_divider(RMT_CLK_DIVIDER);
    let rmt_channel = rmt
        .channel0
        .configure_tx(p.GPIO14, tx_config)
        .expect("Failed to configure RMT TX channel");

    let step_output = RmtStepOutput::new(rmt_channel);
    let dir_pin = Output::new(p.GPIO27, Level::Low, OutputConfig::default());
    let enable_pin = Output::new(p.GPIO26, Level::High, OutputConfig::default());

    let step_dir_config = StepDirConfig::default();
    let step_dir_motor = StepDirMotor::new(step_output, dir_pin, enable_pin, step_dir_config);

    let motor = Motor57AIM::new(step_dir_motor, Motor57AIMConfig::default(), Delay);

    let mut adc1_config = AdcConfig::new();
    let adc_pin = adc1_config.enable_pin(p.GPIO36, Attenuation::_11dB);
    let adc1 = Adc::new(p.ADC1, adc1_config);
    let current_sensor = AdcCurrentSensor::new(adc1, adc_pin);

    static MECHANICAL: MechanicalConfig = MechanicalConfig {
        pulley_teeth: 20,
        belt_pitch_mm: 2.0,
        reverse_direction: false,
    };
    let limits = MotionLimits::default();

    let board = StepDirBoard::new(
        motor,
        current_sensor,
        Delay,
        &MECHANICAL,
        HomingConfig::default(),
    );
    let controller = OSSM.controller(board, limits.clone(), UPDATE_INTERVAL_SECS);

    let sw_int = SoftwareInterruptControl::new(p.SW_INTERRUPT);
    let app_core_stack = APP_CORE_STACK.init(Stack::new());

    let second_core = move || {
        let executor = InterruptExecutor::new(sw_int.software_interrupt2);
        let executor = EXECUTOR_CORE_1.init(executor);
        let spawner = executor.start(Priority::Priority2);

        spawner.spawn(motion_task(controller)).unwrap();

        MOTION_READY.signal(true);

        loop {}
    };

    esp_rtos::start_second_core(
        p.CPU_CTRL,
        sw_int.software_interrupt0,
        sw_int.software_interrupt1,
        app_core_stack,
        second_core,
    );

    MOTION_READY.wait().await;

    info!(
        "Motion task started on core 1 at {}ms interval",
        UPDATE_INTERVAL_SECS * 1000.0
    );

    let radio = &*mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    );

    let connector =
        BleConnector::new(radio, p.BT, Default::default()).expect("Could not create BleConnector");
    ble_remote::start(&spawner, connector, &PATTERNS);

    let mut pattern_runner = PATTERNS.runner(AnyPattern::all_builtin());
    pattern_runner.run(Delay).await;
}

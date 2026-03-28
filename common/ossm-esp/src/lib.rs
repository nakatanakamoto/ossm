#![no_std]

pub use embassy_executor::Spawner;
pub use embassy_sync::blocking_mutex::raw::{CriticalSectionRawMutex, NoopRawMutex};
pub use embassy_sync::mutex::Mutex;
pub use embassy_sync::signal::Signal;
pub use embassy_time::Delay;
pub use embassy_time::{Duration, Ticker};
pub use esp_hal::Blocking;
pub use esp_hal::gpio::{Level, Output, OutputConfig};
pub use esp_hal::interrupt::Priority;
pub use esp_hal::interrupt::software::SoftwareInterruptControl;
pub use esp_hal::system::Stack;
pub use esp_hal::timer::timg::TimerGroup;
pub use esp_hal::uart::{Config as UartConfig, Uart};
pub use esp_radio::ble::controller::BleConnector;
pub use esp_radio::esp_now::{EspNowManager, EspNowSender};
pub use esp_rtos::embassy::InterruptExecutor;
pub use log;
pub use m57aim_motor::{Modbus, Motor57AIM, Motor57AIMConfig};
pub use ossm::{MechanicalConfig, MotionController, MotionLimits, Ossm};
pub use ossm_m5_remote::RemoteConfig;
pub use pattern_engine::{AnyPattern, PatternEngine};
pub use rs485_board::{Rs485, Rs485Board, Rs485ModbusTransport};
pub use static_cell::StaticCell;

pub const UPDATE_INTERVAL_SECS: f64 = 0.01;
pub const MOTOR_BAUD_RATE: u32 = 115_200;
pub const DEVICE_ADDR: u8 = 0x01;

pub type ConcreteTransport =
    Rs485ModbusTransport<Rs485<Uart<'static, Blocking>, Output<'static>>, Delay>;
pub type ConcreteMotor = Motor57AIM<Modbus<ConcreteTransport>, Delay>;
pub type ConcreteBoard = Rs485Board<ConcreteMotor>;

pub static OSSM: Ossm = Ossm::new();
pub static PATTERNS: PatternEngine = PatternEngine::new(&OSSM);
pub static EXECUTOR_CORE_1: StaticCell<InterruptExecutor<2>> = StaticCell::new();
pub static APP_CORE_STACK: StaticCell<Stack<16384>> = StaticCell::new();
pub static MOTION_READY: Signal<CriticalSectionRawMutex, bool> = Signal::new();

static MECHANICAL: MechanicalConfig = MechanicalConfig {
    pulley_teeth: 20,
    belt_pitch_mm: 2.0,
};

/// Allocate a value in a static cell, returning a `&'static mut` reference.
#[macro_export]
macro_rules! mk_static {
    ($t:ty, $val:expr) => {{
        static STATIC_CELL: $crate::StaticCell<$t> = $crate::StaticCell::new();
        STATIC_CELL.init($val)
    }};
}

/// Define the embassy task that ticks the motion controller at a fixed interval.
///
/// Must be invoked at module level - the `#[embassy_executor::task]` attribute
/// requires a top-level function item.
#[macro_export]
macro_rules! define_motion_task {
    () => {
        #[embassy_executor::task]
        async fn motion_task(
            mut controller: $crate::MotionController<'static, $crate::ConcreteBoard>,
        ) {
            let interval_us = ($crate::UPDATE_INTERVAL_SECS * 1_000_000.0) as u64;
            let mut ticker = $crate::Ticker::every($crate::Duration::from_micros(interval_us));

            loop {
                if let Err(e) = controller.update().await {
                    $crate::log::error!("Motion controller fault: {:?}", e);
                }
                ticker.next().await;
            }
        }
    };
}

/// Initialise UART and RS485 with board-specific pin assignments.
///
/// Returns an `Rs485` value ready to pass to [`build_motor`].
/// The DE (driver enable) pin is constructed by the caller since
/// its configuration can vary between boards.
///
/// ```ignore
/// let de = Output::new(p.GPIO11, Level::Low, OutputConfig::default());
/// let rs485 = init_uart!(p, GPIO10, GPIO12, de);
/// ```
#[macro_export]
macro_rules! init_uart {
    ($p:expr, $tx:ident, $rx:ident, $de:expr) => {{
        let uart_config = $crate::UartConfig::default().with_baudrate($crate::MOTOR_BAUD_RATE);
        let uart = $crate::Uart::new($p.UART1, uart_config)
            .expect("Failed to initialize UART")
            .with_tx($p.$tx)
            .with_rx($p.$rx);

        $crate::Rs485::new(uart, $de)
    }};
}

/// Spawn [`motion_task`] on the given spawner.
///
/// Expects `motion_task` to be in scope (defined via [`define_motion_task!`]).
/// Works with any executor - use directly for the default core, or inside
/// [`run_on_core1!`] to run on the second core.
#[macro_export]
macro_rules! launch_motion_controller {
    ($spawner:expr, $controller:expr) => {
        $spawner.spawn(motion_task($controller)).unwrap()
    };
}

/// Run a closure on core 1 using a high-priority interrupt executor.
///
/// The closure receives an [`embassy_executor::Spawner`] bound to the
/// core 1 executor. Blocks core 0 until core 1 signals it is ready.
///
/// ```ignore
/// ossm_esp::run_on_core1!(p.SW_INTERRUPT, p.CPU_CTRL, |spawner| {
///     ossm_esp::launch_motion_controller!(spawner, controller);
/// });
/// ```
#[macro_export]
macro_rules! run_on_core1 {
    ($sw_interrupt:expr, $cpu_ctrl:expr, |$spawner:ident| $body:expr) => {{
        let sw_int = $crate::SoftwareInterruptControl::new($sw_interrupt);
        let app_core_stack = $crate::APP_CORE_STACK.init($crate::Stack::new());

        let second_core = move || {
            let executor = $crate::InterruptExecutor::new(sw_int.software_interrupt2);
            let executor = $crate::EXECUTOR_CORE_1.init(executor);
            let $spawner = executor.start($crate::Priority::Priority2);

            $body;

            $crate::MOTION_READY.signal(true);
            loop {}
        };

        esp_rtos::start_second_core(
            $cpu_ctrl,
            sw_int.software_interrupt0,
            sw_int.software_interrupt1,
            app_core_stack,
            second_core,
        );

        $crate::MOTION_READY.wait().await;
    }};
}

pub fn init_logging(level: log::LevelFilter) {
    ossm::logging::init(level, |line| {
        esp_println::println!("{}", line);
    });
}

pub fn build_motor(rs485: Rs485<Uart<'static, Blocking>, Output<'static>>) -> ConcreteMotor {
    let transport = Rs485ModbusTransport::new(rs485, Delay);
    Motor57AIM::new(
        Modbus::new(transport, DEVICE_ADDR),
        Motor57AIMConfig::default(),
        Delay,
    )
}

pub fn build_controller(
    motor: ConcreteMotor,
    limits: &MotionLimits,
) -> MotionController<'static, ConcreteBoard> {
    let board = Rs485Board::new(motor, &MECHANICAL);
    OSSM.controller(board, limits.clone(), UPDATE_INTERVAL_SECS)
}

pub fn init_radio() -> &'static esp_radio::Controller<'static> {
    &*mk_static!(
        esp_radio::Controller<'static>,
        esp_radio::init().expect("Failed to initialize radio controller")
    )
}

pub use esp_radio::esp_now::EspNowReceiver;

pub struct EspNow {
    pub manager: &'static EspNowManager<'static>,
    pub sender: &'static Mutex<NoopRawMutex, EspNowSender<'static>>,
    pub receiver: EspNowReceiver<'static>,
}

pub fn init_esp_now(
    radio: &'static esp_radio::Controller<'static>,
    wifi: esp_hal::peripherals::WIFI<'static>,
) -> EspNow {
    let (mut wifi_controller, interfaces) =
        esp_radio::wifi::new(radio, wifi, Default::default()).unwrap();
    wifi_controller
        .set_mode(esp_radio::wifi::WifiMode::Sta)
        .unwrap();
    wifi_controller.start().unwrap();

    let esp_now = interfaces.esp_now;
    log::info!("ESP-NOW version {}", esp_now.version().unwrap());

    let (manager, sender, receiver) = esp_now.split();
    let manager = mk_static!(EspNowManager<'static>, manager);
    let sender = mk_static!(
        Mutex::<NoopRawMutex, EspNowSender<'static>>,
        Mutex::<NoopRawMutex, _>::new(sender)
    );

    EspNow {
        manager,
        sender,
        receiver,
    }
}

pub fn start_m5_remote(spawner: &Spawner, esp_now: EspNow, limits: &MotionLimits) {
    let remote_config = RemoteConfig {
        max_velocity_mm_s: limits.max_velocity_mm_s,
        max_travel_mm: limits.max_position_mm - limits.min_position_mm,
    };

    ossm_m5_remote::start(
        spawner,
        esp_now.manager,
        esp_now.sender,
        esp_now.receiver,
        &PATTERNS,
        remote_config,
    );
}

pub fn init_ble(
    radio: &'static esp_radio::Controller<'static>,
    bt: esp_hal::peripherals::BT<'static>,
) -> BleConnector<'static> {
    BleConnector::new(radio, bt, Default::default()).expect("Could not create BleConnector")
}

pub fn start_ble_remote(spawner: &Spawner, connector: BleConnector<'static>) {
    ble_remote::start(spawner, connector, &PATTERNS);
}

pub async fn run_patterns() -> ! {
    let mut runner = PATTERNS.runner(AnyPattern::all_builtin());
    runner.run(Delay).await
}

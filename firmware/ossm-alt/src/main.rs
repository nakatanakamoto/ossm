#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use log::LevelFilter;

extern crate alloc;

use {esp_backtrace as _, esp_println as _};

esp_bootloader_esp_idf::esp_app_desc!();

ossm_esp::define_motion_task!();

#[esp_rtos::main]
async fn main(spawner: ossm_esp::Spawner) {
    ossm_esp::init_logging(LevelFilter::Info);

    ossm::build_info!();

    let p = esp_hal::init(esp_hal::Config::default());
    esp_alloc::heap_allocator!(size: 128 * 1024);

    let timg0 = ossm_esp::TimerGroup::new(p.TIMG0);
    esp_rtos::start(timg0.timer0);

    // Manual DE control — hardware RS485 mode has inverted RTS polarity
    // on the OSSM Alt board, so we toggle a GPIO directly instead.
    let de = ossm_esp::Output::new(
        p.GPIO11,
        ossm_esp::Level::Low,
        ossm_esp::OutputConfig::default(),
    );
    let rs485 = ossm_esp::init_uart!(p, GPIO10, GPIO12, de);

    let limits = ossm_esp::MotionLimits::default();
    let motor = ossm_esp::build_motor(rs485);
    let controller = ossm_esp::build_controller(motor, &limits);

    ossm_esp::run_on_core1!(p.SW_INTERRUPT, p.CPU_CTRL, |spawner| {
        ossm_esp::launch_motion_controller!(spawner, controller);
    });

    let radio = ossm_esp::init_radio();

    let esp_now = ossm_esp::init_esp_now(radio, p.WIFI);
    ossm_esp::start_m5_remote(&spawner, esp_now, &limits);

    let ble = ossm_esp::init_ble(radio, p.BT);
    ossm_esp::start_ble_remote(&spawner, ble);

    ossm_esp::run_patterns().await;
}

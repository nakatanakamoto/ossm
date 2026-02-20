#![no_std]
#![no_main]

use embassy_executor::Spawner;
use esp_backtrace as _;
use m57aim_motor::M57AIMMotor;
use ossm_alt_board::OssmAltBoard;

#[esp_hal_embassy::main]
async fn main(_spawner: Spawner) {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    let board = OssmAltBoard::new(peripherals);
    let mut motor = M57AIMMotor::new(board.uart);

    loop {}
}

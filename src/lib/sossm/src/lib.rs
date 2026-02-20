#![no_std]

mod motor;
mod board;

pub use motor::Motor;
pub use board::Board;

pub struct Sossm {}

impl Sossm {
    pub fn new() -> Self {
        Self {}
    }

    pub fn run(&self) {
        // Run the Sossm system
    }
}

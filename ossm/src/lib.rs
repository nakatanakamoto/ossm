#![no_std]
extern crate alloc;

mod command;
mod limits;
mod mechanical;
mod motion;
mod motor;

pub use command::{
    Command, CommandChannel, HomingSignal, MotionCommand, MoveCompleteSignal, OssmChannels,
};
pub use limits::MotionLimits;
pub use mechanical::MechanicalConfig;
pub use motion::MotionController;
pub use motor::{Motor, MotorTelemetry};

/// Lightweight command handle for application code.
///
/// `Ossm` sends commands to a [`MotionController`] via a shared channel.
/// All methods take `&self` and are safe to call from any context — no mutex
/// or critical section needed.
///
/// Create both halves with [`Ossm::new()`], then hand the
/// [`MotionController`] to an interrupt or timer task.
pub struct Ossm<'a> {
    channels: &'a OssmChannels,
    update_interval_secs: f64,
}

impl<'a> Ossm<'a> {
    /// Create a `Ossm` command handle and a [`MotionController`] engine,
    /// both connected to the given channels.
    ///
    /// The returned `MotionController` should be spawned on an
    /// `InterruptExecutor` via [`MotionController::update()`].
    pub fn new<M: Motor>(
        motor: M,
        config: &MechanicalConfig,
        limits: MotionLimits,
        update_interval_secs: f64,
        channels: &'a OssmChannels,
    ) -> (Self, MotionController<'a, M>) {
        let controller =
            MotionController::new(motor, config, limits, update_interval_secs, channels);
        let handle = Self {
            channels,
            update_interval_secs,
        };
        (handle, controller)
    }

    pub fn update_interval_secs(&self) -> f64 {
        self.update_interval_secs
    }

    pub fn enable(&self) {
        let _ = self.channels.commands.try_send(Command::Enable);
    }

    pub fn disable(&self) {
        let _ = self.channels.commands.try_send(Command::Disable);
    }

    /// Send a Home command and wait for homing to complete.
    pub async fn home(&self) {
        self.channels.homing_done.reset();
        let _ = self.channels.commands.try_send(Command::Home);
        self.channels.homing_done.wait().await;
    }

    /// Move to a position expressed as a fraction of the machine range (0.0–1.0).
    pub fn move_to(&self, position: f64) {
        let _ = self.channels.commands.try_send(Command::MoveTo(position));
    }

    /// Set velocity as a fraction of max velocity (0.0–1.0).
    pub fn set_speed(&self, speed: f64) {
        let _ = self.channels.commands.try_send(Command::SetSpeed(speed));
    }

    /// Send a combined motion command (position + velocity). Fire-and-forget.
    pub fn push_motion(&self, cmd: MotionCommand) {
        let _ = self.channels.commands.try_send(Command::Motion(cmd));
    }

    /// Wait for the current move to complete (Moving → Ready).
    pub async fn wait_move_complete(&self) {
        self.channels.move_complete.wait().await;
    }
}

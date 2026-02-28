#![no_std]
extern crate alloc;

mod command;
mod limits;
mod mechanical;
mod motion;
mod motor;

pub use command::{Command, CommandChannel, HomingSignal, MotionCommand, MoveCompleteSignal};
pub use limits::MotionLimits;
pub use mechanical::MechanicalConfig;
pub use motion::MotionController;
pub use motor::{Motor, MotorTelemetry};

/// Lightweight command handle for application code.
///
/// `Sossm` sends commands to a [`MotionController`] via a shared channel.
/// All methods take `&self` and are safe to call from any context — no mutex
/// or critical section needed.
///
/// Create both halves with [`Sossm::new()`], then hand the
/// [`MotionController`] to an interrupt or timer task.
pub struct Sossm<'a> {
    commands: &'a CommandChannel,
    homing_done: &'a HomingSignal,
    move_complete: &'a MoveCompleteSignal,
    update_interval_secs: f64,
}

impl<'a> Sossm<'a> {
    /// Create a `Sossm` command handle and a [`MotionController`] engine,
    /// both connected to the given `commands` channel.
    ///
    /// The returned `MotionController` should be spawned on an
    /// `InterruptExecutor` via [`MotionController::update()`].
    pub fn new<M: Motor>(
        motor: M,
        config: &MechanicalConfig,
        limits: MotionLimits,
        update_interval_secs: f64,
        commands: &'a CommandChannel,
        homing_done: &'a HomingSignal,
        move_complete: &'a MoveCompleteSignal,
    ) -> (Self, MotionController<'a, M>) {
        let controller = MotionController::new(
            motor,
            config,
            limits,
            update_interval_secs,
            commands,
            homing_done,
            move_complete,
        );
        let handle = Self {
            commands,
            homing_done,
            move_complete,
            update_interval_secs,
        };
        (handle, controller)
    }

    pub fn update_interval_secs(&self) -> f64 {
        self.update_interval_secs
    }

    pub fn enable(&self) {
        let _ = self.commands.try_send(Command::Enable);
    }

    pub fn disable(&self) {
        let _ = self.commands.try_send(Command::Disable);
    }

    /// Send a Home command and wait for homing to complete.
    pub async fn home(&self) {
        self.homing_done.reset();
        let _ = self.commands.try_send(Command::Home);
        self.homing_done.wait().await;
    }

    /// Move to a position expressed as a fraction of the machine range (0.0–1.0).
    pub fn move_to(&self, position: f64) {
        let _ = self.commands.try_send(Command::MoveTo(position));
    }

    /// Set velocity as a fraction of max velocity (0.0–1.0).
    pub fn set_speed(&self, speed: f64) {
        let _ = self.commands.try_send(Command::SetSpeed(speed));
    }

    /// Send a combined motion command (position + velocity). Fire-and-forget.
    pub fn push_motion(&self, cmd: MotionCommand) {
        let _ = self.commands.try_send(Command::Motion(cmd));
    }

    /// Wait for the current move to complete (Moving → Ready).
    pub async fn wait_move_complete(&self) {
        self.move_complete.wait().await;
    }
}

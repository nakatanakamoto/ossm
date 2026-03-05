use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embassy_sync::signal::Signal;

pub type CommandChannel = Channel<CriticalSectionRawMutex, Command, 8>;
pub type HomingSignal = Signal<CriticalSectionRawMutex, ()>;
pub type MoveCompleteSignal = Signal<CriticalSectionRawMutex, ()>;

/// Bundles the core channels shared between the [`Ossm`](crate::Ossm) handle
/// and the [`MotionController`](crate::MotionController).
///
/// Declare a single `static` in your firmware instead of three separate ones:
///
/// ```ignore
/// static CHANNELS: OssmChannels = OssmChannels::new();
/// ```
pub struct OssmChannels {
    pub commands: CommandChannel,
    pub homing_done: HomingSignal,
    pub move_complete: MoveCompleteSignal,
}

impl OssmChannels {
    pub const fn new() -> Self {
        Self {
            commands: CommandChannel::new(),
            homing_done: HomingSignal::new(),
            move_complete: MoveCompleteSignal::new(),
        }
    }
}

/// A single motion command expressed as fractions of the machine range.
///
/// Unlike the separate `MoveTo`/`SetSpeed` commands, this sets both atomically.
/// The `MotionController` converts fractions to mm internally.
#[derive(Debug, Clone, Copy)]
pub struct MotionCommand {
    /// Target position as a fraction of the machine range (0.0–1.0).
    pub position: f64,
    /// Velocity as a fraction of max velocity (0.0–1.0).
    pub speed: f64,
    /// Torque limit as a percentage (0–100). `None` uses the motor default.
    /// Ignored until `Motor` gains a `set_torque()` method.
    pub torque: Option<f64>,
}

#[derive(Debug, Clone, Copy)]
pub enum Command {
    Enable,
    Disable,
    Home,
    MoveTo(f64),
    SetSpeed(f64),
    Motion(MotionCommand),
}

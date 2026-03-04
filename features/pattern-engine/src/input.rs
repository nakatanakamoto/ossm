use core::cell::Cell;

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::blocking_mutex::Mutex;

/// Live input that controls pattern behavior.
///
/// Written by the UI/BLE task, read by the pattern via [`PatternCtx`].
/// All values are fractions of the machine range / max velocity.
#[derive(Debug, Clone, Copy)]
pub struct PatternInput {
    /// Maximum depth as a fraction of the machine range (0.0–1.0).
    pub depth: f64,
    /// Stroke length as a fraction of the machine range (0.0–1.0).
    /// Shallowest point = `depth - stroke`.
    pub stroke: f64,
    /// Velocity as a fraction of max velocity (0.0–1.0).
    pub velocity: f64,
    /// Sensation value (-1.0 to 1.0). Meaning is pattern-specific.
    pub sensation: f64,
}

impl PatternInput {
    pub const DEFAULT: Self = Self {
        depth: 0.5,
        stroke: 0.4,
        velocity: 0.5,
        sensation: 0.0,
    };
}

impl Default for PatternInput {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Cross-task shared pattern input.
///
/// Uses a blocking mutex around a `Cell` for instant lock-free reads.
/// The critical section disables interrupts for nanoseconds — negligible
/// overhead on ESP32-S3.
///
/// Declare as a static in the firmware crate:
/// ```ignore
/// static PATTERN_INPUT: SharedPatternInput = SharedPatternInput::new(Cell::new(PatternInput::DEFAULT));
/// ```
pub type SharedPatternInput = Mutex<CriticalSectionRawMutex, Cell<PatternInput>>;

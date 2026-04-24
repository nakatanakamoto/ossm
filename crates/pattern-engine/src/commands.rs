//! Public API for remote adapters — the full surface a new remote needs.

use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::pubsub::{self, Subscriber};

use crate::{AnyPattern, EngineState, PatternEngine, PatternInput, PatternMeta};

#[derive(Debug, Clone, Copy)]
pub enum PlaybackCommand {
    Play(usize),
    Pause,
    Resume,
    Stop,
    Home,
}

/// Input values are clamped to their valid range on dispatch.
#[derive(Debug, Clone, Copy)]
pub enum InputCommand {
    /// 0.0–1.0 (fraction of max velocity).
    SetSpeed(f64),
    /// 0.0–1.0 (fraction of depth).
    SetStroke(f64),
    /// 0.0–1.0 (fraction of machine range).
    SetDepth(f64),
    /// -1.0–1.0 (pattern-specific).
    SetSensation(f64),
}

pub fn dispatch_playback(engine: &PatternEngine, cmd: PlaybackCommand) {
    match cmd {
        PlaybackCommand::Play(idx) => engine.play(idx),
        PlaybackCommand::Pause => engine.pause(),
        PlaybackCommand::Resume => engine.resume(),
        PlaybackCommand::Stop => engine.stop(),
        PlaybackCommand::Home => engine.home(),
    }
}

pub fn dispatch_input(engine: &PatternEngine, cmd: InputCommand) {
    engine.input().sender().send_modify(|opt| {
        if let Some(input) = opt {
            match cmd {
                InputCommand::SetSpeed(v) => input.velocity = v.clamp(0.0, 1.0),
                InputCommand::SetStroke(v) => input.stroke = v.clamp(0.0, 1.0),
                InputCommand::SetDepth(v) => input.depth = v.clamp(0.0, 1.0),
                InputCommand::SetSensation(v) => input.sensation = v.clamp(-1.0, 1.0),
            }
        }
    });
}

pub fn current_state(engine: &PatternEngine) -> EngineState {
    engine.state()
}

pub fn current_input(engine: &PatternEngine) -> PatternInput {
    engine.input().try_get().unwrap_or(PatternInput::DEFAULT)
}

pub fn pattern_list() -> &'static [PatternMeta] {
    &AnyPattern::BUILTIN_PATTERNS
}

pub fn pattern_description(idx: usize) -> Option<&'static str> {
    AnyPattern::BUILTIN_PATTERNS
        .get(idx)
        .map(|m| m.description)
}

pub type StateSubscriber<'a> = Subscriber<'a, CriticalSectionRawMutex, EngineState, 1, 8, 0>;

pub fn subscribe_state(engine: &PatternEngine) -> Result<StateSubscriber<'_>, pubsub::Error> {
    engine.state_subscriber()
}

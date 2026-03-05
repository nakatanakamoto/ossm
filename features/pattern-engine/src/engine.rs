use embassy_futures::select::{self, Either};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_hal_async::delay::DelayNs;
use ossm::OssmChannels;

use crate::any_pattern::AnyPattern;
use crate::input::SharedPatternInput;
use crate::pattern::{Pattern, PatternCtx};

#[derive(Debug, Clone, Copy)]
pub enum EngineCommand {
    Play(usize),
    Pause,
    Resume,
    Stop,
}

/// Channel for sending commands to the engine.
///
/// Capacity of 4 is sufficient: commands are processed one at a time
/// and senders are typically a single UI/BLE task.
pub type EngineCommandChannel = Channel<CriticalSectionRawMutex, EngineCommand, 4>;

#[derive(Debug, Clone, Copy)]
enum EngineState {
    Idle,
    Playing(usize),
    Paused(usize),
}

pub struct PatternEngine<const N: usize> {
    patterns: [AnyPattern; N],
    state: EngineState,
}

impl<const N: usize> PatternEngine<N> {
    pub fn new(patterns: [AnyPattern; N]) -> Self {
        Self {
            patterns,
            state: EngineState::Idle,
        }
    }

    pub fn pattern_count(&self) -> usize {
        N
    }

    pub fn pattern_name(&self, index: usize) -> Option<&'static str> {
        self.patterns.get(index).map(|p| p.name())
    }

    pub fn pattern_description(&self, index: usize) -> Option<&'static str> {
        self.patterns.get(index).map(|p| p.description())
    }

    pub fn pattern_list(&self) -> impl Iterator<Item = (usize, &'static str, &'static str)> + '_ {
        self.patterns
            .iter()
            .enumerate()
            .map(|(i, p)| (i, p.name(), p.description()))
    }

    /// Run the engine forever, processing commands and driving patterns.
    ///
    /// This method never returns. It should be the last `.await` in the
    /// pattern task, or spawned as a dedicated async task.
    ///
    /// `delay` must implement `Clone` so a fresh [`PatternCtx`] can be created
    /// each time a pattern starts. All embassy `Delay` types are `Copy`.
    pub async fn run<D: DelayNs + Clone>(
        &mut self,
        engine_commands: &EngineCommandChannel,
        channels: &'static OssmChannels,
        input: &'static SharedPatternInput,
        delay: D,
    ) -> ! {
        loop {
            match self.state {
                EngineState::Idle | EngineState::Paused(_) => {
                    let cmd = engine_commands.receive().await;
                    self.handle_command(cmd);
                }
                EngineState::Playing(idx) => {
                    let mut ctx = PatternCtx::new(channels, input, delay.clone());

                    let result = select::select(
                        self.patterns[idx].run(&mut ctx),
                        engine_commands.receive(),
                    )
                    .await;

                    match result {
                        Either::First(()) => {
                            // Pattern returned (unusual — they normally loop forever).
                            self.state = EngineState::Idle;
                        }
                        Either::Second(cmd) => {
                            self.handle_command(cmd);
                        }
                    }
                }
            }
        }
    }

    fn handle_command(&mut self, cmd: EngineCommand) {
        match cmd {
            EngineCommand::Play(idx) => {
                if idx < N {
                    self.state = EngineState::Playing(idx);
                }
            }
            EngineCommand::Pause => {
                if let EngineState::Playing(idx) = self.state {
                    self.state = EngineState::Paused(idx);
                }
            }
            EngineCommand::Resume => {
                if let EngineState::Paused(idx) = self.state {
                    self.state = EngineState::Playing(idx);
                }
            }
            EngineCommand::Stop => {
                self.state = EngineState::Idle;
            }
        }
    }
}

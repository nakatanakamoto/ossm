use core::fmt::Debug;

use alloc::fmt;
use esp_hal::{
    Blocking,
    gpio::Level,
    rmt::{Channel, PulseCode, Tx},
};
use ossm::StepOutput;

/// RMT clock divider. With an statically set 80 MHz base clock,
/// divider=4 gives 20 MHz (50 ns per tick).
pub const RMT_CLK_DIVIDER: u8 = 4;

/// Step pulse width in RMT ticks. At 50 ns/tick, 20 ticks = 1 µs per half.
/// Full step period = 2 µs → max 500 kHz step rate.
/// Matches FastAccelStepper's minimum 1 µs pulse width used by the reference firmware.
const STEP_PULSE_TICKS: u16 = 20;

/// Maximum number of step pulses per RMT transmission batch.
/// The ESP32 has 512 × 32-bit entries shared across 8 channels (64 per channel).
/// We use 63 pulses + 1 end marker per batch.
const STEP_BATCH_SIZE: usize = 63;

/// Generates step pulses via the ESP32 RMT (Remote Control Transceiver) peripheral.
///
/// The RMT is a hardware pulse generator. You give it an array of
/// [`PulseCode`]s — each one describes two pin-level/duration pairs — and it
/// drives the GPIO autonomously. No CPU involvement while pulses are in flight.
///
/// Each `PulseCode` encodes one complete step pulse:
/// ```text
/// PulseCode::new(High, 20, Low, 20)
///   → pin HIGH for 20 ticks (1 µs), then LOW for 20 ticks (1 µs)
///   → one step = 2 µs total
/// ```
///
/// The RMT has limited on-chip RAM (64 entries per channel on ESP32), so large
/// step counts are sent in batches of 63 + end marker. The hardware supports
/// wrap-around (ping-pong) mode to refill one half while transmitting the
/// other, but esp-hal 1.0.0 doesn't expose this on ESP32 — so we batch
/// sequentially with a brief CPU gap (~µs) between batches. In practice this
/// gap is negligible and doesn't affect motor behaviour.
/// This also matches the reference firmware's approach of sending steps.
pub struct RmtStepOutput {
    channel: Option<Channel<'static, Blocking, Tx>>,
    pulse: PulseCode,
}

impl RmtStepOutput {
    pub fn new(channel: Channel<'static, Blocking, Tx>) -> Self {
        let pulse = PulseCode::new(Level::High, STEP_PULSE_TICKS, Level::Low, STEP_PULSE_TICKS);
        Self {
            channel: Some(channel),
            pulse,
        }
    }
}

pub enum RmtError {
    Rmt(esp_hal::rmt::Error),
    ChannelInUse,
}

impl Debug for RmtError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rmt(e) => write!(f, "Rmt({:?})", e),
            Self::ChannelInUse => write!(f, "ChannelInUse"),
        }
    }
}

impl StepOutput for RmtStepOutput {
    type Error = RmtError;

    /// Send the specified number of step pulses, if greater than the
    /// batch size, in sequential batches of 63 + end marker. The RMT transmits
    /// each batch autonomously while the CPU prepares the next batch.
    async fn step(&mut self, count: u32) -> Result<(), Self::Error> {
        let mut remaining = count as usize;
        let mut ch = self.channel.take().ok_or(RmtError::ChannelInUse)?;

        while remaining > 0 {
            let batch = remaining.min(STEP_BATCH_SIZE);
            remaining -= batch;

            let mut buf = [PulseCode::end_marker(); STEP_BATCH_SIZE + 1];
            for entry in buf.iter_mut().take(batch) {
                *entry = self.pulse;
            }
            buf[batch] = PulseCode::end_marker();

            let tx = ch.transmit(&buf[..batch + 1]).map_err(RmtError::Rmt)?;
            ch = match tx.wait() {
                Ok(c) => c,
                Err((e, c)) => {
                    self.channel = Some(c);
                    return Err(RmtError::Rmt(e));
                }
            };
        }

        self.channel = Some(ch);
        Ok(())
    }
}

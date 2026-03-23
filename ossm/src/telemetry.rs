use embassy_futures::select;
use embassy_time::{Duration, Ticker};
use telemetry::protocol::FrameType;
use telemetry::transport::TelemetrySender;

use crate::Ossm;
use crate::state::MotionPhase;
use crate::state::MotionState;

/// Wire-format telemetry payload for core motion state (device → browser).
///
/// Fixed 15-byte layout:
/// ```text
/// position: f32 LE  [0..4]
/// velocity: f32 LE  [4..8]
/// torque:   f32 LE  [8..12]
/// phase:    u8       [12]
/// sequence: u16 LE   [13..15]
/// ```
#[derive(Debug, Clone, Copy)]
pub struct CoreTelemetry {
    pub position: f32,
    pub velocity: f32,
    pub torque: f32,
    pub phase: MotionPhase,
    pub sequence: u16,
}

impl CoreTelemetry {
    pub const SIZE: usize = 15;

    pub fn from_state(state: &MotionState, sequence: u16) -> Self {
        Self {
            position: state.position,
            velocity: state.velocity,
            torque: state.torque,
            phase: state.phase,
            sequence,
        }
    }

    pub fn to_bytes(&self, buf: &mut [u8; Self::SIZE]) {
        buf[0..4].copy_from_slice(&self.position.to_le_bytes());
        buf[4..8].copy_from_slice(&self.velocity.to_le_bytes());
        buf[8..12].copy_from_slice(&self.torque.to_le_bytes());
        buf[12] = self.phase.to_u8();
        buf[13..15].copy_from_slice(&self.sequence.to_le_bytes());
    }

    pub fn from_bytes(buf: &[u8; Self::SIZE]) -> Self {
        Self {
            position: f32::from_le_bytes([buf[0], buf[1], buf[2], buf[3]]),
            velocity: f32::from_le_bytes([buf[4], buf[5], buf[6], buf[7]]),
            torque: f32::from_le_bytes([buf[8], buf[9], buf[10], buf[11]]),
            phase: MotionPhase::from_u8(buf[12]),
            sequence: u16::from_le_bytes([buf[13], buf[14]]),
        }
    }
}

/// Produces core telemetry frames, emitting only when state changes.
///
/// Wakes on phase transitions (via pubsub) or a 20ms poll tick.
/// Skips sending if nothing has changed since the last frame.
pub async fn telemetry_task(ossm: &'static Ossm, sender: &TelemetrySender) -> ! {
    let mut phase_sub = ossm.phase_subscriber().expect("No subscriber slots available");
    let mut ticker = Ticker::every(Duration::from_millis(20));
    let mut sequence: u16 = 0;
    let mut last_state: Option<MotionState> = None;

    loop {
        select::select(phase_sub.next_message_pure(), ticker.next()).await;

        let state = ossm.motion_state();

        if let Some(prev) = &last_state {
            if state.phase == prev.phase
                && state.position == prev.position
                && state.velocity == prev.velocity
                && state.torque == prev.torque
            {
                continue;
            }
        }

        last_state = Some(state);

        let telem = CoreTelemetry::from_state(&state, sequence);
        let mut payload = [0u8; CoreTelemetry::SIZE];
        telem.to_bytes(&mut payload);
        let _ = sender.send(FrameType::CoreTelemetry, &payload);

        sequence = sequence.wrapping_add(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_telemetry_round_trip() {
        let original = CoreTelemetry {
            position: 0.75,
            velocity: 0.5,
            torque: 1.0,
            phase: MotionPhase::Moving,
            sequence: 42,
        };

        let mut payload = [0u8; CoreTelemetry::SIZE];
        original.to_bytes(&mut payload);

        let decoded = CoreTelemetry::from_bytes(&payload);
        assert_eq!(decoded.position, original.position);
        assert_eq!(decoded.velocity, original.velocity);
        assert_eq!(decoded.torque, original.torque);
        assert_eq!(decoded.phase, original.phase);
        assert_eq!(decoded.sequence, original.sequence);
    }
}

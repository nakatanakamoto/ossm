use crc::{CRC_8_SMBUS, Crc};

const CRC: Crc<u8> = Crc::<u8>::new(&CRC_8_SMBUS);

/// Maximum payload size for any single frame (core or feature-specific).
pub const MAX_PAYLOAD: usize = 64;

/// Central registry of frame type IDs on the wire.
///
/// Every frame type across all features must be registered here to
/// guarantee uniqueness at compile time.
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameType {
    // Core (0x01–0x0F)
    CoreTelemetry = 0x01,
    // Feature frame types (0x10–0x1F)
}

impl FrameType {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0x01 => Some(Self::CoreTelemetry),
            _ => None,
        }
    }
}

/// Maximum encoded frame size before COBS encoding.
/// type(1) + payload(up to MAX_PAYLOAD) + crc(1)
const MAX_RAW_FRAME: usize = 1 + MAX_PAYLOAD + 1;

/// Maximum COBS-encoded frame size including the trailing zero delimiter.
/// COBS expands at most 1 byte per 254 input bytes + 1 delimiter.
pub const MAX_ENCODED_FRAME: usize = MAX_RAW_FRAME + (MAX_RAW_FRAME / 254) + 2;

/// Encode a frame: [type] [payload...] [crc8], then COBS-encode.
///
/// Returns the number of bytes written to `out`, including the trailing
/// zero delimiter.
pub fn encode_frame(frame_type: u8, payload: &[u8], out: &mut [u8]) -> Result<usize, EncodeError> {
    let raw_len = 1 + payload.len() + 1; // type + payload + crc
    if raw_len > MAX_RAW_FRAME {
        return Err(EncodeError::PayloadTooLarge);
    }

    let mut raw = [0u8; MAX_RAW_FRAME];
    raw[0] = frame_type;
    raw[1..1 + payload.len()].copy_from_slice(payload);

    let mut digest = CRC.digest();
    digest.update(&raw[..1 + payload.len()]);
    raw[1 + payload.len()] = digest.finalize();

    let cobs_len = cobs::encode(&raw[..raw_len], out);
    // Append zero delimiter
    if cobs_len + 1 > out.len() {
        return Err(EncodeError::BufferTooSmall);
    }
    out[cobs_len] = 0x00;

    Ok(cobs_len + 1)
}

/// Decode a COBS-encoded frame (without trailing zero delimiter).
///
/// Returns `(frame_type, payload_len)` with the payload written into `payload_out`.
pub fn decode_frame(cobs_data: &[u8], payload_out: &mut [u8]) -> Result<(u8, usize), DecodeError> {
    let mut raw = [0u8; MAX_RAW_FRAME];
    let report = cobs::decode(cobs_data, &mut raw).map_err(|_| DecodeError::InvalidCobs)?;
    let raw_len = report.frame_size();

    if raw_len < 2 {
        return Err(DecodeError::TooShort);
    }

    let frame_type = raw[0];
    let payload_len = raw_len - 2; // minus type and crc
    let received_crc = raw[raw_len - 1];

    let mut digest = CRC.digest();
    digest.update(&raw[..raw_len - 1]);
    let computed_crc = digest.finalize();

    if received_crc != computed_crc {
        return Err(DecodeError::CrcMismatch);
    }

    if payload_len > payload_out.len() {
        return Err(DecodeError::PayloadTooLarge);
    }

    payload_out[..payload_len].copy_from_slice(&raw[1..1 + payload_len]);
    Ok((frame_type, payload_len))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncodeError {
    PayloadTooLarge,
    BufferTooSmall,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    InvalidCobs,
    TooShort,
    CrcMismatch,
    PayloadTooLarge,
}

#[cfg(test)]
mod tests {
    use ossm::{CoreTelemetry, MotionPhase};

    use super::*;

    #[test]
    fn frame_encode_decode_round_trip() {
        let telemetry = CoreTelemetry {
            position: 0.25,
            velocity: 0.8,
            torque: 0.6,
            phase: MotionPhase::Paused,
            sequence: 1000,
        };

        let mut payload = [0u8; CoreTelemetry::SIZE];
        telemetry.to_bytes(&mut payload);

        let mut encoded = [0u8; MAX_ENCODED_FRAME];
        let len = encode_frame(FrameType::CoreTelemetry as u8, &payload, &mut encoded).unwrap();

        let cobs_data = &encoded[..len - 1];
        let mut decoded_payload = [0u8; MAX_PAYLOAD];
        let (frame_type, payload_len) = decode_frame(cobs_data, &mut decoded_payload).unwrap();

        assert_eq!(frame_type, FrameType::CoreTelemetry as u8);
        assert_eq!(payload_len, CoreTelemetry::SIZE);

        let decoded =
            CoreTelemetry::from_bytes(decoded_payload[..CoreTelemetry::SIZE].try_into().unwrap());
        assert_eq!(decoded.position, telemetry.position);
        assert_eq!(decoded.velocity, telemetry.velocity);
        assert_eq!(decoded.phase, telemetry.phase);
        assert_eq!(decoded.sequence, telemetry.sequence);
    }

    #[test]
    fn corrupt_crc_rejected() {
        let payload = [0x42; 8];

        let mut encoded = [0u8; MAX_ENCODED_FRAME];
        let len = encode_frame(FrameType::CoreTelemetry as u8, &payload, &mut encoded).unwrap();

        encoded[1] ^= 0xFF;

        let cobs_data = &encoded[..len - 1];
        let mut decoded_payload = [0u8; MAX_PAYLOAD];
        let result = decode_frame(cobs_data, &mut decoded_payload);

        assert!(matches!(
            result,
            Err(DecodeError::CrcMismatch) | Err(DecodeError::InvalidCobs)
        ));
    }
}

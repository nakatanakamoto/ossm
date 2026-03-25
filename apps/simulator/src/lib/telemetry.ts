/**
 * Telemetry protocol decoder matching the firmware's COBS + CRC8 wire format.
 *
 * Wire format: [COBS-encoded frame][0x00 delimiter]
 * Decoded frame: [frame_type: u8][payload: N bytes][crc8: u8]
 *
 * CoreTelemetry payload (15 bytes, little-endian):
 *   [0..4]   f32  position  (0.0–1.0)
 *   [4..8]   f32  velocity  (0.0–1.0)
 *   [8..12]  f32  torque    (0.0–1.0)
 *   [12]     u8   phase     (MotionPhase enum)
 *   [13..15] u16  sequence  (wrapping counter)
 */

export const enum FrameType {
  CoreTelemetry = 0x01,
}

export const enum MotionPhase {
  Disabled = 0,
  Enabled = 1,
  Ready = 2,
  Moving = 3,
  Stopping = 4,
  Paused = 5,
}

export interface CoreTelemetryPayload {
  position: number;
  velocity: number;
  torque: number;
  phase: MotionPhase;
  sequence: number;
}

// CRC-8-SMBUS lookup table (polynomial 0x07)
const CRC8_TABLE = new Uint8Array(256);
{
  for (let i = 0; i < 256; i++) {
    let crc = i;
    for (let bit = 0; bit < 8; bit++) {
      crc = crc & 0x80 ? (crc << 1) ^ 0x07 : crc << 1;
    }
    CRC8_TABLE[i] = crc & 0xff;
  }
}

function crc8(data: Uint8Array, start: number, length: number): number {
  let crc = 0x00;
  for (let i = start; i < start + length; i++) {
    crc = CRC8_TABLE[(crc ^ data[i]) & 0xff];
  }
  return crc;
}

/** COBS-decode in place. Returns the decoded length, or -1 on error. */
function cobsDecode(encoded: Uint8Array, length: number): number {
  if (length === 0) return 0;

  let readIdx = 0;
  let writeIdx = 0;

  while (readIdx < length) {
    const code = encoded[readIdx++];
    if (code === 0) return -1; // unexpected zero in COBS data

    const dataLen = code - 1;
    if (readIdx + dataLen > length) return -1;

    for (let i = 0; i < dataLen; i++) {
      encoded[writeIdx++] = encoded[readIdx++];
    }

    // If code < 0xFF, there's an implicit zero (unless we've consumed everything)
    if (code < 0xff && readIdx < length) {
      encoded[writeIdx++] = 0x00;
    }
  }

  return writeIdx;
}

function parseCoreTelemetry(payload: Uint8Array): CoreTelemetryPayload {
  const view = new DataView(
    payload.buffer,
    payload.byteOffset,
    payload.byteLength,
  );
  return {
    position: view.getFloat32(0, true),
    velocity: view.getFloat32(4, true),
    torque: view.getFloat32(8, true),
    phase: payload[12] as MotionPhase,
    sequence: view.getUint16(13, true),
  };
}

export type TelemetryFrame =
  | { type: "telemetry"; payload: CoreTelemetryPayload }
  | { type: "log"; text: string };

/**
 * Accumulates raw bytes from the wire and yields decoded telemetry frames
 * or ASCII log lines. Splits on 0x00 delimiter: binary packets are decoded
 * as COBS telemetry frames, printable ASCII segments are passed through as logs.
 */
export class TelemetryDecoder {
  private buffer = new Uint8Array(256);
  private pos = 0;

  push(chunk: Uint8Array): TelemetryFrame[] {
    const frames: TelemetryFrame[] = [];

    for (let i = 0; i < chunk.length; i++) {
      const byte = chunk[i];

      if (byte === 0x00) {
        if (this.pos > 0) {
          const frame = this.processPacket();
          if (frame) frames.push(frame);
          this.pos = 0;
        }
        continue;
      }

      // Grow buffer if needed
      if (this.pos >= this.buffer.length) {
        const next = new Uint8Array(this.buffer.length * 2);
        next.set(this.buffer);
        this.buffer = next;
      }

      this.buffer[this.pos++] = byte;
    }

    return frames;
  }

  private processPacket(): TelemetryFrame | null {
    const raw = this.buffer.subarray(0, this.pos);

    // If all bytes are printable ASCII (0x0A, 0x0D, 0x20-0x7E), treat as log line
    if (this.isAscii(raw)) {
      const text = new TextDecoder().decode(raw).trim();
      if (text.length > 0) {
        return { type: "log", text };
      }
      return null;
    }

    // Otherwise attempt COBS + CRC decode
    const work = new Uint8Array(this.pos);
    work.set(raw);

    const decoded = cobsDecode(work, this.pos);
    if (decoded < 2) return null; // need at least frame_type + crc

    const frameType = work[0];
    const payloadLen = decoded - 2; // exclude type and crc bytes
    const receivedCrc = work[decoded - 1];
    const computedCrc = crc8(work, 0, decoded - 1);

    if (receivedCrc !== computedCrc) return null;

    if (frameType === FrameType.CoreTelemetry && payloadLen === 15) {
      const payload = parseCoreTelemetry(work.subarray(1, 16));
      return { type: "telemetry", payload };
    }

    return null;
  }

  private isAscii(data: Uint8Array): boolean {
    for (let i = 0; i < data.length; i++) {
      const b = data[i];
      if (b === 0x0a || b === 0x0d || (b >= 0x20 && b <= 0x7e)) continue;
      return false;
    }
    return true;
  }
}

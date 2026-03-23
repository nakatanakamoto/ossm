use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::Channel;
use embedded_io_async::Write;
use log::error;

use crate::protocol::{self, EncodeError, FrameType, MAX_ENCODED_FRAME};

/// A pre-encoded frame ready for transmission.
pub struct EncodedFrame {
    buf: [u8; MAX_ENCODED_FRAME],
    len: usize,
}

/// Channel for submitting frames to the telemetry writer.
///
/// Capacity of 8 frames. Producers use [`TelemetrySender::send`] which
/// drops the frame silently if the channel is full.
pub type TelemetryChannel = Channel<CriticalSectionRawMutex, EncodedFrame, 8>;

/// Handle for features to send telemetry frames.
pub struct TelemetrySender {
    channel: &'static TelemetryChannel,
}

impl TelemetrySender {
    pub fn new(channel: &'static TelemetryChannel) -> Self {
        Self { channel }
    }

    /// Encode and enqueue a frame. Drops silently if the channel is full.
    pub fn send(&self, frame_type: FrameType, payload: &[u8]) -> Result<(), EncodeError> {
        let mut frame = EncodedFrame {
            buf: [0u8; MAX_ENCODED_FRAME],
            len: 0,
        };

        frame.len = protocol::encode_frame(frame_type as u8, payload, &mut frame.buf)?;
        let _ = self.channel.try_send(frame);

        Ok(())
    }
}

/// Writer task: drains the channel and writes frames to the wire.
pub async fn tx_task<W: Write>(
    mut writer: W,
    channel: &'static TelemetryChannel,
) -> ! {
    loop {
        let frame = channel.receive().await;

        if let Err(e) = writer.write_all(&frame.buf[..frame.len]).await {
            error!("Telemetry TX write error: {:?}", e);
        }
    }
}

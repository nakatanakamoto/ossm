use crate::command::{StateCommand, StateResponse};
use crate::{MotionCommand, Ossm};

impl Ossm {
    /// Try to read a pending motion command from the channel.
    pub fn try_recv_motion(&self) -> Option<MotionCommand> {
        self.channels.move_cmd.try_receive().ok()
    }

    /// Signal that the current motion has completed.
    pub fn signal_motion_complete(&self) {
        self.channels.move_resp.signal(Ok(()));
    }

    /// Signal that the current state command has completed.
    pub fn respond_state(&self, resp: StateResponse) {
        self.channels.state_resp.signal(resp);
    }

    /// Try to read a pending state command from the channel.
    pub fn try_recv_state(&self) -> Option<StateCommand> {
        self.channels.state_cmd.try_receive().ok()
    }
}

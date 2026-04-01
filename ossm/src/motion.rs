use crate::command::{Cancelled, MotionCommand, OssmChannels, StateCommand, StateResponse};
use crate::planner::{MotionPlanner, PlannerState};
use crate::{Board, MotionLimits};

#[derive(Debug, Clone, Copy, PartialEq)]
enum BoardState {
    Disabled,
    Enabled,
    Ready,
}

/// What the controller should do when the planner finishes decelerating.
#[derive(Debug, Clone, Copy)]
enum PendingAction {
    Disable,
    Home,
}

pub struct MotionController<'a, B: Board> {
    board: B,
    channels: &'a OssmChannels,
    board_state: BoardState,
    limits: MotionLimits,
    planner: MotionPlanner,
    torque: Option<f64>,
    pending_action: Option<PendingAction>,
}

impl<'a, B: Board> MotionController<'a, B> {
    pub(crate) fn new(
        board: B,
        limits: MotionLimits,
        update_interval_secs: f64,
        channels: &'a OssmChannels,
    ) -> Self {
        let range = limits.max_position_mm - limits.min_position_mm;
        let planner = MotionPlanner::new(
            limits.max_velocity_mm_s / range,
            limits.max_acceleration_mm_s2 / range,
            limits.max_jerk_mm_s3 / range,
            update_interval_secs,
        );

        Self {
            board,
            channels,
            board_state: BoardState::Disabled,
            limits,
            planner,
            torque: None,
            pending_action: None,
        }
    }

    pub async fn update(&mut self) -> Result<(), B::Error> {
        if let Err(e) = self.board.tick().await {
            log::error!("Board tick fault: {:?}", e);
            self.enter_fault();
            return Err(e);
        }

        self.tick().await?;

        if let Ok(cmd) = self.channels.state_cmd.try_receive() {
            self.process_state_command(cmd).await?;
        }

        if let Ok(cmd) = self.channels.move_cmd.try_receive() {
            self.process_move_command(cmd).await;
        }

        Ok(())
    }

    async fn process_state_command(&mut self, cmd: StateCommand) -> Result<(), B::Error> {
        if self.board_state == BoardState::Disabled {
            return match cmd {
                StateCommand::Enable => match self.board.enable().await {
                    Ok(()) => {
                        self.board_state = BoardState::Enabled;
                        self.respond(StateResponse::Completed);
                        Ok(())
                    }
                    Err(e) => {
                        log::error!("Board enable failed: {:?}", e);
                        self.respond(StateResponse::Fault);
                        Err(e)
                    }
                },
                StateCommand::Disable => {
                    self.respond(StateResponse::Completed);
                    Ok(())
                }
                _ => {
                    self.respond(StateResponse::InvalidTransition);
                    Ok(())
                }
            };
        }

        match (self.planner.state(), cmd) {
            (_, StateCommand::Enable) => {
                self.respond(StateResponse::Completed);
            }

            (PlannerState::Idle, StateCommand::Disable) => {
                self.disable().await;
                self.respond(StateResponse::Completed);
            }
            (PlannerState::Paused, StateCommand::Disable) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.planner.reset();
                self.disable().await;
                self.respond(StateResponse::Completed);
            }
            (PlannerState::Moving, StateCommand::Disable) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.pending_action = Some(PendingAction::Disable);
                self.planner.stop(false);
            }
            (PlannerState::Stopping { .. }, StateCommand::Disable) => {
                self.pending_action = Some(PendingAction::Disable);
                self.planner.stop(false);
            }

            (PlannerState::Idle, StateCommand::Home) => {
                match self.home().await {
                    Ok(()) => self.respond(StateResponse::Completed),
                    Err(e) => {
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                }
            }
            (PlannerState::Moving, StateCommand::Home) => {
                self.channels.move_resp.signal(Err(Cancelled));
                self.pending_action = Some(PendingAction::Home);
                self.planner.stop(false);
            }
            (PlannerState::Paused, StateCommand::Home) => {
                self.channels.move_resp.signal(Err(Cancelled));
                match self.home().await {
                    Ok(()) => self.respond(StateResponse::Completed),
                    Err(e) => {
                        self.respond(StateResponse::Fault);
                        return Err(e);
                    }
                }
            }

            (PlannerState::Moving, StateCommand::Pause) => {
                self.planner.stop(true);
                self.respond(StateResponse::Completed);
            }

            (PlannerState::Paused, StateCommand::Resume) => {
                self.planner.resume();
                self.apply_torque().await;
                self.respond(StateResponse::Completed);
            }

            _ => {
                self.respond(StateResponse::InvalidTransition);
            }
        }

        Ok(())
    }

    async fn process_move_command(&mut self, cmd: MotionCommand) {
        if self.board_state != BoardState::Ready
            || !matches!(self.planner.state(), PlannerState::Idle | PlannerState::Moving)
        {
            return;
        }

        self.planner.set_target(cmd.position, cmd.speed);
        self.torque = cmd.torque;
        self.apply_torque().await;
    }

    async fn tick(&mut self) -> Result<(), B::Error> {
        let prev = self.planner.state();
        if let Some(out) = self.planner.step() {
            let range = self.limits.max_position_mm - self.limits.min_position_mm;
            let mm = self.limits.min_position_mm + out.position * range;
            if let Err(e) = self.board.set_position(mm).await {
                log::error!("Board set_position failed: {:?}", e);
            }
        }
        let curr = self.planner.state();

        if prev != curr {
            self.on_transition(prev, curr).await?;
        }

        Ok(())
    }

    async fn on_transition(
        &mut self,
        from: PlannerState,
        to: PlannerState,
    ) -> Result<(), B::Error> {
        match (from, to) {
            (PlannerState::Stopping { .. }, PlannerState::Idle) => {
                match self.pending_action.take() {
                    Some(PendingAction::Disable) => {
                        self.disable().await;
                        self.respond(StateResponse::Completed);
                    }
                    Some(PendingAction::Home) => {
                        match self.home().await {
                            Ok(()) => self.respond(StateResponse::Completed),
                            Err(e) => {
                                self.respond(StateResponse::Fault);
                                return Err(e);
                            }
                        }
                    }
                    None => {
                        self.channels.move_resp.signal(Ok(()));
                    }
                }
            }
            (PlannerState::Moving, PlannerState::Idle) => {
                self.channels.move_resp.signal(Ok(()));
            }
            _ => {}
        }
        Ok(())
    }

    async fn home(&mut self) -> Result<(), B::Error> {
        if let Err(e) = self.board.home().await {
            log::error!("Board home failed: {:?}", e);
            return Err(e);
        }

        self.planner.home();

        if let Err(e) = self
            .board
            .set_position(self.limits.min_position_mm)
            .await
        {
            log::error!("Board set_position after home failed: {:?}", e);
            return Err(e);
        }

        self.board_state = BoardState::Ready;
        Ok(())
    }

    async fn disable(&mut self) {
        if let Err(e) = self.board.disable().await {
            log::error!("Board disable failed: {:?}", e);
        }
        self.planner.reset();
        self.pending_action = None;
        self.board_state = BoardState::Disabled;
    }

    fn enter_fault(&mut self) {
        match self.planner.state() {
            PlannerState::Moving | PlannerState::Paused => {
                self.channels.move_resp.signal(Err(Cancelled));
            }
            PlannerState::Stopping { preserve_target: true } => {
                self.channels.move_resp.signal(Err(Cancelled));
            }
            PlannerState::Stopping { preserve_target: false } => {
                if self.pending_action.is_some() {
                    self.respond(StateResponse::Fault);
                }
            }
            _ => {}
        }
        self.planner.reset();
        self.pending_action = None;
        self.board_state = BoardState::Disabled;
    }

    fn respond(&self, resp: StateResponse) {
        self.channels.state_resp.signal(resp);
    }

    async fn apply_torque(&mut self) {
        let fraction = self.torque.unwrap_or(1.0);
        if let Err(e) = self.board.set_torque(fraction).await {
            log::error!("Board set_torque failed: {:?}", e);
        }
    }
}

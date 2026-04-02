use rsruckig::prelude::*;

const MIN_VELOCITY: f64 = 0.001;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlannerState {
    Idle,
    Moving,
    /// Decelerating to zero velocity. `preserve_target` controls whether
    /// the motion target is kept for a later `resume()` (`true` = pause)
    /// or discarded (`false` = full stop).
    Stopping { preserve_target: bool },
    Paused,
}

#[derive(Debug, Clone, Copy)]
pub struct TrajectoryStep {
    pub position: f64,
    #[allow(dead_code)]
    pub velocity: f64,
    #[allow(dead_code)]
    pub acceleration: f64,
}

#[derive(Debug, Clone, Copy)]
struct MotionTarget {
    position: f64,
    velocity: f64,
}

pub struct MotionPlanner {
    state: PlannerState,
    max_velocity: f64,
    target: Option<MotionTarget>,
    ruckig: Ruckig<1, ThrowErrorHandler>,
    input: InputParameter<1>,
    output: OutputParameter<1>,
}

impl MotionPlanner {
    pub fn new(
        max_velocity: f64,
        max_acceleration: f64,
        max_jerk: f64,
        update_interval_secs: f64,
    ) -> Self {
        let mut input = InputParameter::new(None);
        input.current_position[0] = 0.0;
        input.target_position[0] = 0.0;
        input.max_velocity[0] = MIN_VELOCITY;
        input.max_acceleration[0] = max_acceleration;
        input.max_jerk[0] = max_jerk;
        input.synchronization = Synchronization::None;
        input.duration_discretization = DurationDiscretization::Discrete;

        Self {
            state: PlannerState::Idle,
            max_velocity,
            target: None,
            ruckig: Ruckig::<1, ThrowErrorHandler>::new(None, update_interval_secs),
            input,
            output: OutputParameter::new(None),
        }
    }

    pub fn state(&self) -> PlannerState {
        self.state
    }

    /// Advance the trajectory by one tick.
    ///
    /// Returns the computed position, velocity, and acceleration for this
    /// step, or `None` if no trajectory is active (`Idle` or `Paused`).
    ///
    /// State transitions happen internally when Ruckig reports `Finished`:
    /// - `Stopping { preserve_target: true }` → `Paused`
    /// - `Stopping { preserve_target: false }` / `Moving` → `Idle`
    pub fn step(&mut self) -> Option<TrajectoryStep> {
        if !matches!(self.state, PlannerState::Moving | PlannerState::Stopping { .. }) {
            return None;
        }

        let Ok(result) = self.ruckig.update(&self.input, &mut self.output) else {
            log::error!("Ruckig update failed, resetting planner");
            self.reset();
            return None;
        };

        if !matches!(result, RuckigResult::Working | RuckigResult::Finished) {
            log::error!("Ruckig returned unexpected result: {:?}, resetting planner", result);
            self.reset();
            return None;
        }

        let position = self.output.new_position[0].clamp(0.0, 1.0);
        let velocity = self.output.new_velocity[0];
        let acceleration = self.output.new_acceleration[0];

        self.output.pass_to_input(&mut self.input);

        if result == RuckigResult::Finished {
            match self.state {
                PlannerState::Stopping {
                    preserve_target: true,
                } => {
                    self.state = PlannerState::Paused;
                }
                _ => {
                    self.target = None;
                    self.state = PlannerState::Idle;
                }
            }
        }

        Some(TrajectoryStep {
            position,
            velocity,
            acceleration,
        })
    }

    /// Set a new motion target. Transitions `Idle → Moving` or updates an
    /// in-flight trajectory. Ignored during `Stopping` or `Paused`.
    pub fn set_target(&mut self, position: f64, velocity: f64) {
        if !matches!(self.state, PlannerState::Idle | PlannerState::Moving) {
            return;
        }

        if !position.is_finite() || !velocity.is_finite() {
            return;
        }

        let position = position.clamp(0.0, 1.0);
        let velocity = velocity.clamp(MIN_VELOCITY, self.max_velocity);

        self.target = Some(MotionTarget { position, velocity });
        self.sync_ruckig();

        if self.state == PlannerState::Idle {
            self.state = PlannerState::Moving;
        }
    }

    /// Initiate jerk-limited deceleration to zero velocity.
    ///
    /// When `preserve_target` is `true`, the motion target is kept so a
    /// later `resume()` can continue from where it stopped (pause semantics).
    /// When `false`, the target is discarded on completion (full stop).
    pub fn stop(&mut self, preserve_target: bool) {
        if !matches!(self.state, PlannerState::Moving | PlannerState::Stopping { .. }) {
            return;
        }
        self.input.control_interface = ControlInterface::Velocity;
        self.input.target_velocity[0] = 0.0;
        self.output.time = 0.0;
        self.state = PlannerState::Stopping { preserve_target };
    }

    /// Resume from `Paused`: restore the preserved target and continue.
    pub fn resume(&mut self) {
        if self.state != PlannerState::Paused {
            return;
        }
        self.input.control_interface = ControlInterface::Position;
        self.sync_ruckig();
        self.state = PlannerState::Moving;
    }

    /// Set the current position without starting motion. Only valid in `Idle`.
    pub fn set_position(&mut self, position: f64) {
        if self.state != PlannerState::Idle || !position.is_finite() {
            return;
        }
        let position = position.clamp(0.0, 1.0);
        self.input.current_position[0] = position;
        self.input.target_position[0] = position;
    }

    /// Reset position to 0, clear target, go `Idle`.
    pub fn home(&mut self) {
        self.input.control_interface = ControlInterface::Position;
        self.input.current_position[0] = 0.0;
        self.input.target_position[0] = 0.0;
        self.input.current_velocity[0] = 0.0;
        self.input.current_acceleration[0] = 0.0;
        self.target = None;
        self.state = PlannerState::Idle;
    }

    /// Force the planner back to `Idle`, fully resetting Ruckig state.
    pub(crate) fn reset(&mut self) {
        self.home();
    }

    fn sync_ruckig(&mut self) {
        if let Some(target) = &self.target {
            self.input.target_position[0] = target.position;
            self.input.max_velocity[0] = target.velocity;
            self.output.time = 0.0;
            self.ruckig.reset();
        }
    }
}

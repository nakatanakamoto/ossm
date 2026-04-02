use core::sync::atomic::{AtomicI32, Ordering};
use core::future::Future;
use core::pin::pin;
use core::task::{Context, Waker};

use embassy_time::{Delay, Duration, Ticker};
use embedded_hal_async::delay::DelayNs;
extern crate alloc;
use alloc::string::String;

use alloc::vec::Vec;

use ossm::{MechanicalConfig, MotionLimits, MotionPlanner, Ossm, PlannerState, StateCommand, StateResponse};
use pattern_engine::{AnyPattern, PatternEngine, PatternInput};
use sim_board::SimBoard;
use sim_motor::SimMotor;
use wasm_bindgen::prelude::*;

static OSSM: Ossm = Ossm::new();
static PATTERNS: PatternEngine = PatternEngine::new(&OSSM);
static MOTOR_POSITION: AtomicI32 = AtomicI32::new(0);

static MECHANICAL: MechanicalConfig = MechanicalConfig {
    pulley_teeth: 20,
    belt_pitch_mm: 2.0,
};

#[wasm_bindgen]
pub struct Simulator {
    steps_per_mm: f64,
    min_position_mm: f64,
    max_position_mm: f64,
}

#[wasm_bindgen]
impl Simulator {
    /// Create a new simulator and start the motion + pattern tasks.
    ///
    /// `update_interval_ms` controls the motion controller tick rate (e.g. 10.0 for 10ms).
    #[wasm_bindgen(constructor)]
    pub fn new(update_interval_ms: f64) -> Self {
        ossm::logging::init(log::LevelFilter::Info, |line| {
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(line));
        });

        ossm::build_info!();

        let update_interval_secs = update_interval_ms / 1000.0;
        let motor = SimMotor::new(&MOTOR_POSITION);
        let board = SimBoard::new(motor, &MECHANICAL);

        let limits = MotionLimits {
            min_position_mm: 10.0,
            max_position_mm: 250.0,
            ..MotionLimits::default()
        };

        let mut controller = OSSM.controller(board, limits.clone(), update_interval_secs);

        let interval_us = (update_interval_secs * 1_000_000.0) as u64;

        wasm_bindgen_futures::spawn_local(async move {
            let mut ticker = Ticker::every(Duration::from_micros(interval_us));
            loop {
                if let Err(e) = controller.update().await {
                    log::error!("Motion controller fault: {:?}", e);
                }
                ticker.next().await;
            }
        });

        let mut pattern_runner = PATTERNS.runner(AnyPattern::all_builtin());

        wasm_bindgen_futures::spawn_local(async move {
            pattern_runner.run(Delay).await;
        });

        let steps_per_mm = MECHANICAL.steps_per_mm(SimMotor::STEPS_PER_REV) as f64;

        Self {
            steps_per_mm,
            min_position_mm: limits.min_position_mm,
            max_position_mm: limits.max_position_mm,
        }
    }

    /// Engine state: 0 = idle, 1 = homing, 2 = playing, 3 = paused.
    pub fn get_engine_state(&self) -> u8 {
        PATTERNS.state().as_u8()
    }

    /// Current position as a fraction of the machine range (0.0-1.0).
    pub fn get_position(&self) -> f64 {
        let steps = MOTOR_POSITION.load(Ordering::Relaxed);
        let mm = steps as f64 / self.steps_per_mm;
        let range = self.max_position_mm - self.min_position_mm;
        (mm - self.min_position_mm) / range
    }

    /// Set the maximum depth as a fraction of the machine range (0.0-1.0).
    pub fn set_depth(&self, depth: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.depth = depth;
            }
        });
    }

    /// Set the stroke length as a fraction of the machine range (0.0-1.0).
    pub fn set_stroke(&self, stroke: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.stroke = stroke;
            }
        });
    }

    /// Set velocity as a fraction of max velocity (0.0-1.0).
    pub fn set_velocity(&self, velocity: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.velocity = velocity;
            }
        });
    }

    /// Set sensation value (-1.0 to 1.0). Meaning is pattern-specific.
    pub fn set_sensation(&self, sensation: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.sensation = sensation;
            }
        });
    }

    pub fn play(&self, index: usize) {
        PATTERNS.play(index);
    }

    pub fn pause(&self) {
        PATTERNS.pause();
    }

    pub fn resume(&self) {
        PATTERNS.resume();
    }

    pub fn stop(&self) {
        PATTERNS.stop();
    }

    pub fn pattern_count(&self) -> usize {
        AnyPattern::BUILTIN_PATTERNS.len()
    }

    pub fn pattern_name(&self, index: usize) -> String {
        AnyPattern::BUILTIN_PATTERNS
            .get(index)
            .map(|p| String::from(p.name))
            .unwrap_or_default()
    }

    pub fn pattern_description(&self, index: usize) -> String {
        AnyPattern::BUILTIN_PATTERNS
            .get(index)
            .map(|p| String::from(p.description))
            .unwrap_or_default()
    }
}

#[wasm_bindgen]
pub struct MotionGraph {
    planner: MotionPlanner,
}

#[wasm_bindgen]
impl MotionGraph {
    #[wasm_bindgen(constructor)]
    pub fn new(
        max_velocity: f64,
        max_acceleration: f64,
        max_jerk: f64,
        update_interval_ms: f64,
    ) -> Self {
        Self {
            planner: MotionPlanner::new(
                max_velocity,
                max_acceleration,
                max_jerk,
                update_interval_ms / 1000.0,
            ),
        }
    }

    /// Set a motion target using fractions (0.0–1.0).
    pub fn set_target(&mut self, position: f64, speed: f64) {
        self.planner.set_target(position, speed);
    }

    pub fn stop(&mut self) {
        self.planner.stop(false);
    }

    pub fn home(&mut self) {
        self.planner.home();
    }

    /// 0 = idle, 1 = moving, 2 = stopping, 3 = paused.
    pub fn state(&self) -> u8 {
        match self.planner.state() {
            ossm::PlannerState::Idle => 0,
            ossm::PlannerState::Moving => 1,
            ossm::PlannerState::Stopping { .. } => 2,
            ossm::PlannerState::Paused => 3,
        }
    }

    /// Run up to `count` steps. Returns a flat `Float64Array`:
    /// `[pos, vel, accel, pos, vel, accel, ...]`.
    ///
    /// Stops early if the planner reaches `Idle` or `Paused`.
    pub fn run_steps(&mut self, count: u32) -> Vec<f64> {
        let mut result = Vec::with_capacity(count as usize * 3);
        for _ in 0..count {
            if let Some(out) = self.planner.step() {
                result.push(out.position);
                result.push(out.velocity);
                result.push(out.acceleration);
            } else {
                break;
            }
        }
        result
    }
}

/// Simulated delay that yields one step per timestep of elapsed time.
///
/// The default `DelayNs` implementations break `delay_ms`/`delay_us`
/// into many small `delay_ns` calls. We accumulate nanoseconds across
/// calls and yield once per simulated timestep, so a 5000ms delay at
/// a 10ms timestep produces exactly 500 yields (= 500 recorded steps
/// of flat position).
#[derive(Clone)]
struct SimulatedDelay {
    step_ns: u64,
    accumulated_ns: u64,
}

impl SimulatedDelay {
    fn new(timestep_ms: f64) -> Self {
        Self {
            step_ns: (timestep_ms * 1_000_000.0) as u64,
            accumulated_ns: 0,
        }
    }
}

impl DelayNs for SimulatedDelay {
    async fn delay_ns(&mut self, ns: u32) {
        self.accumulated_ns += ns as u64;
        while self.accumulated_ns >= self.step_ns {
            self.accumulated_ns -= self.step_ns;
            let mut yielded = false;
            core::future::poll_fn(move |cx| {
                if yielded {
                    core::task::Poll::Ready(())
                } else {
                    yielded = true;
                    cx.waker().wake_by_ref();
                    core::task::Poll::Pending
                }
            })
            .await;
        }
    }
}

#[wasm_bindgen]
pub struct PatternRecorder {
    ossm: &'static Ossm,
    engine: &'static PatternEngine,
    max_velocity: f64,
    max_acceleration: f64,
    max_jerk: f64,
}

#[wasm_bindgen]
impl PatternRecorder {
    #[wasm_bindgen(constructor)]
    pub fn new(
        max_velocity: f64,
        max_acceleration: f64,
        max_jerk: f64,
    ) -> Self {
        let ossm = Box::leak(Box::new(Ossm::new()));
        let engine = Box::leak(Box::new(PatternEngine::new(ossm)));
        Self { ossm, engine, max_velocity, max_acceleration, max_jerk }
    }

    pub fn pattern_count(&self) -> usize {
        AnyPattern::BUILTIN_PATTERNS.len()
    }

    pub fn pattern_name(&self, index: usize) -> String {
        AnyPattern::BUILTIN_PATTERNS
            .get(index)
            .map(|p| String::from(p.name))
            .unwrap_or_default()
    }

    /// Record a pattern's trajectory.
    ///
    /// Returns a flat `Float64Array`: `[pos, vel, accel, pos, vel, accel, ...]`.
    pub fn record(
        &self,
        pattern: usize,
        depth: f64,
        stroke: f64,
        velocity: f64,
        sensation: f64,
        timestep_ms: f64,
        steps: u32,
    ) -> Vec<f64> {
        self.ossm.drain();

        self.engine.input().sender().send(PatternInput {
            depth,
            stroke,
            velocity,
            sensation,
        });

        let mut runner = self.engine.runner(AnyPattern::all_builtin());
        let run_fut = runner.run(SimulatedDelay::new(timestep_ms));
        let mut run_fut = pin!(run_fut);

        self.engine.play(pattern);

        let mut planner = MotionPlanner::new(
            self.max_velocity,
            self.max_acceleration,
            self.max_jerk,
            timestep_ms / 1000.0,
        );
        let rest_position = depth * (1.0 - stroke);
        planner.set_position(rest_position);

        let mut was_moving = false;
        let mut last_pos = rest_position;

        let waker = Waker::noop();
        let mut cx = Context::from_waker(&waker);

        let mut result = Vec::with_capacity(steps as usize * 3);

        for _ in 0..steps {
            // Poll the pattern runner — it may send commands to the Ossm
            let _ = run_fut.as_mut().poll(&mut cx);

            // Process state commands (enable, home, etc.)
            while let Some(cmd) = self.ossm.try_receive_state() {
                match cmd {
                    StateCommand::Enable | StateCommand::Disable => {
                        self.ossm.respond_state(StateResponse::Completed);
                    }
                    StateCommand::Home => {
                        planner.home();
                        planner.set_position(rest_position);
                        last_pos = rest_position;
                        self.ossm.respond_state(StateResponse::Completed);
                    }
                    StateCommand::Pause => {
                        planner.stop(true);
                        self.ossm.respond_state(StateResponse::Completed);
                    }
                    StateCommand::Resume => {
                        planner.resume();
                        self.ossm.respond_state(StateResponse::Completed);
                    }
                }
                // Re-poll so the runner can react to the response
                let _ = run_fut.as_mut().poll(&mut cx);
            }

            // Process move commands
            if let Some(cmd) = self.ossm.try_receive_move() {
                planner.set_target(cmd.position, cmd.speed);
                was_moving = true;
            }

            // Step the trajectory
            let prev = planner.state();
            if let Some(out) = planner.step() {
                last_pos = out.position;
                result.push(out.position);
                result.push(out.velocity);
                result.push(out.acceleration);
            } else {
                result.push(last_pos);
                result.push(0.0);
                result.push(0.0);
            }

            // If the planner just finished a move, signal completion
            if was_moving && planner.state() == PlannerState::Idle && prev != PlannerState::Idle {
                self.ossm.complete_move();
                was_moving = false;
                // Re-poll so the runner can start the next move
                let _ = run_fut.as_mut().poll(&mut cx);
            }
        }

        result
    }
}

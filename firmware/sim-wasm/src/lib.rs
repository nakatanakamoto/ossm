extern crate alloc;
use alloc::string::String;

mod recorder;

use embassy_time::{Delay, Duration, Ticker};
use ossm::planner::RuckigPlanner;
use ossm::{MechanicalConfig, MotionLimits, Ossm};
use pattern_engine::{AnyPattern, PatternEngine, PatternInput, SharedPatternInput};
use recorder::PatternRecorder;
use sim_board::SimBoard;
use sim_motor::SimMotor;
use wasm_bindgen::prelude::*;

// --- Simulator (real-time, async, for 3D visualization) ---

static OSSM: Ossm = Ossm::new();
static PATTERNS: PatternEngine = PatternEngine::new(&OSSM);

static MECHANICAL: MechanicalConfig = MechanicalConfig {
    pulley_teeth: 20,
    belt_pitch_mm: 2.0,
};

#[wasm_bindgen]
pub struct Simulator {}

#[wasm_bindgen]
impl Simulator {
    #[wasm_bindgen(constructor)]
    pub fn new(update_interval_ms: f64) -> Self {
        ossm::logging::init(log::LevelFilter::Info, |line| {
            web_sys::console::log_1(&wasm_bindgen::JsValue::from_str(line));
        });

        ossm::build_info!();

        let update_interval_secs = update_interval_ms / 1000.0;
        let motor = SimMotor::new();
        let board = SimBoard::new(motor, &MECHANICAL);

        let limits = MotionLimits {
            min_position_mm: 10.0,
            max_position_mm: 250.0,
            ..MotionLimits::default()
        };

        let mut controller = OSSM.controller(board, limits, update_interval_secs);

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

        Self {}
    }

    pub fn get_engine_state(&self) -> u8 {
        PATTERNS.state().as_u8()
    }

    pub fn get_position(&self) -> f32 {
        OSSM.motion_state().position
    }

    pub fn set_depth(&self, depth: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.depth = depth;
            }
        });
    }

    pub fn set_stroke(&self, stroke: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.stroke = stroke;
            }
        });
    }

    pub fn set_velocity(&self, velocity: f64) {
        PATTERNS.input().sender().send_modify(|opt| {
            if let Some(input) = opt {
                input.velocity = velocity;
            }
        });
    }

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

static RECORDER_OSSM: Ossm = Ossm::new();
static RECORDER_INPUT: SharedPatternInput = SharedPatternInput::new();

const LIMITS: MotionLimits = MotionLimits::DEFAULT;
const RANGE_MM: f64 = LIMITS.max_position_mm - LIMITS.min_position_mm;

#[wasm_bindgen]
pub struct TrajectoryRecorder {}

#[wasm_bindgen]
impl TrajectoryRecorder {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        Self {}
    }

    pub fn min_position_mm(&self) -> f64 {
        LIMITS.min_position_mm
    }

    pub fn max_position_mm(&self) -> f64 {
        LIMITS.max_position_mm
    }

    /// Record a trajectory returning three `Float32Array`s: position,
    /// velocity, and acceleration (all in the 0–1 domain).
    pub fn record(
        &self,
        pattern: usize,
        depth: f64,
        stroke: f64,
        velocity: f64,
        sensation: f64,
        timestep_ms: f64,
        max_samples: usize,
    ) -> TrajectoryResult {
        let timestep_secs = timestep_ms / 1000.0;

        let mut planner = RuckigPlanner::new(
            LIMITS.max_velocity_mm_s / RANGE_MM,
            LIMITS.max_acceleration_mm_s2 / RANGE_MM,
            LIMITS.max_jerk_mm_s3 / RANGE_MM,
            timestep_secs,
        );

        let mut patterns = AnyPattern::all_builtin();
        let Some(pat) = patterns.get_mut(pattern) else {
            return TrajectoryResult::empty();
        };

        let input = PatternInput {
            depth,
            stroke,
            velocity,
            sensation,
        };

        let rest_position = depth * (1.0 - stroke);

        let recorder = PatternRecorder::new(&RECORDER_OSSM, &RECORDER_INPUT);
        let samples = recorder.record(
            pat,
            &mut planner,
            input,
            rest_position,
            timestep_ms,
            max_samples,
        );

        let mut position = alloc::vec::Vec::with_capacity(samples.len());
        let mut velocity = alloc::vec::Vec::with_capacity(samples.len());
        let mut acceleration = alloc::vec::Vec::with_capacity(samples.len());

        for s in &samples {
            position.push(s.position as f32);
            velocity.push(s.velocity as f32);
            acceleration.push(s.acceleration as f32);
        }

        TrajectoryResult {
            position: position.into_boxed_slice(),
            velocity: velocity.into_boxed_slice(),
            acceleration: acceleration.into_boxed_slice(),
        }
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
}

#[wasm_bindgen]
pub struct TrajectoryResult {
    position: Box<[f32]>,
    velocity: Box<[f32]>,
    acceleration: Box<[f32]>,
}

#[wasm_bindgen]
impl TrajectoryResult {
    #[wasm_bindgen(getter)]
    pub fn position(&self) -> Box<[f32]> {
        self.position.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn velocity(&self) -> Box<[f32]> {
        self.velocity.clone()
    }

    #[wasm_bindgen(getter)]
    pub fn acceleration(&self) -> Box<[f32]> {
        self.acceleration.clone()
    }

    fn empty() -> Self {
        Self {
            position: Box::new([]),
            velocity: Box::new([]),
            acceleration: Box::new([]),
        }
    }
}

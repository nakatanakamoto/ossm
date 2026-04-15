extern crate alloc;
use alloc::string::String;

mod recorder;

use ossm::planner::RuckigPlanner;
use ossm::{MotionLimits, Ossm};
use pattern_engine::{AnyPattern, PatternInput, SharedPatternInput};
use recorder::PatternRecorder;
use wasm_bindgen::prelude::*;

static RECORDER_OSSM: Ossm = Ossm::new();
static RECORDER_INPUT: SharedPatternInput = SharedPatternInput::new();

const LIMITS: MotionLimits = MotionLimits::DEFAULT;
const RANGE_MM: f64 = LIMITS.max_position_mm - LIMITS.min_position_mm;

// Matches firmware UPDATE_INTERVAL_SECS so the graph reflects what hardware actually does.
const TIMESTEP_MS: f64 = 10.0;

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

    pub fn timestep_ms(&self) -> f64 {
        TIMESTEP_MS
    }

    /// Record a trajectory returning three `Float32Array`s: position,
    /// velocity, and acceleration (all in the 0-1 domain).
    pub fn record(
        &self,
        pattern: usize,
        depth: f64,
        stroke: f64,
        velocity: f64,
        sensation: f64,
        max_samples: usize,
    ) -> TrajectoryResult {
        let timestep_secs = TIMESTEP_MS / 1000.0;

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
            TIMESTEP_MS,
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

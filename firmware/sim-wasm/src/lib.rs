extern crate alloc;
use alloc::string::String;

use embassy_time::{Delay, Duration, Ticker};
use ossm::{MechanicalConfig, MotionLimits, Ossm};
use pattern_engine::{AnyPattern, PatternEngine};
use sim_board::SimBoard;
use sim_motor::SimMotor;
use wasm_bindgen::prelude::*;

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

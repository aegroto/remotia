use std::time::{Duration, Instant};

use remotia::{
    common::feedback::FeedbackMessage, server::profiling::ServerProfiler, traits::FrameProcessor,
    types::FrameData,
};

use async_trait::async_trait;
use log::info;

pub struct ConsoleAverageStatsLogger {
    header: Option<String>,
    values_to_log: Vec<String>,
    round_duration: Duration,

    current_round_start: Instant,

    logged_frames: Vec<FrameData>,

    log_errors: bool,
}

impl Default for ConsoleAverageStatsLogger {
    fn default() -> Self {
        Self {
            header: None,
            values_to_log: Vec::new(),
            round_duration: Duration::from_secs(1),
            current_round_start: Instant::now(),
            logged_frames: Vec::new(),
            log_errors: false,
        }
    }
}

impl ConsoleAverageStatsLogger {
    pub fn new() -> Self {
        Self::default()
    }

    // Building functions
    pub fn header(mut self, header: &str) -> Self {
        self.header = Some(header.to_string());
        self
    }

    pub fn log(mut self, value: &str) -> Self {
        self.values_to_log.push(value.to_string());
        self
    }

    // Logging functions
    fn print_round_stats(&self) {
        if self.header.is_some() {
            info!("{}", self.header.as_ref().unwrap());
        }

        let logged_frames_count = self.logged_frames.len() as u128;

        if logged_frames_count == 0 {
            info!("No successfully transmitted frames");
            return;
        } else {
            info!("Logged frames: {}", logged_frames_count);
        }

        self.values_to_log.iter().for_each(|value| {
            let avg = self
                .logged_frames
                .iter()
                .map(|frame| get_frame_stat(frame, value))
                .sum::<u128>()
                / logged_frames_count;

            info!("Average {}: {}", value, avg);
        });
    }

    fn reset_round(&mut self) {
        self.logged_frames.clear();
        self.current_round_start = Instant::now();
    }

    fn log_frame_data(&mut self, frame_data: &FrameData) {
        if !self.log_errors && frame_data.get_drop_reason().is_some() {
            return;
        }

        self.logged_frames.push(frame_data.clone_without_buffers());

        if self.current_round_start.elapsed().gt(&self.round_duration) {
            self.print_round_stats();
            self.reset_round();
        }
    }
}

#[async_trait]
impl FrameProcessor for ConsoleAverageStatsLogger {
    async fn process(&mut self, frame_data: FrameData) -> Option<FrameData> {
        self.log_frame_data(&frame_data);
        Some(frame_data)
    }
}

// retro-compatibility for silo pipeline
#[async_trait]
impl ServerProfiler for ConsoleAverageStatsLogger {
    fn log_frame(&mut self, frame_data: FrameData) {
        self.log_frame_data(&frame_data);
    }

    async fn pull_feedback(&mut self) -> Option<FeedbackMessage> {
        None
    }
}

fn get_frame_stat(frame: &FrameData, key: &str) -> u128 {
    if frame.has(key) {
        frame.get(key)
    } else {
        frame.get(key)
    }
}

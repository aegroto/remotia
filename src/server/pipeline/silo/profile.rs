use std::time::{Duration, Instant};

use bytes::BytesMut;
use log::debug;
use tokio::{
    sync::{
        broadcast::Sender,
        mpsc::{Receiver, UnboundedReceiver},
    },
    task::JoinHandle,
};

use crate::{common::{feedback::ServerFeedbackMessage, helpers::silo::channel_pull}, server::{
        profiling::{TransmissionRoundStats, TransmittedFrameStats, ServerProfiler},
        utils::profilation::setup_round_stats,
    }};

use super::transfer::TransferResult;

pub fn launch_profile_thread(
    mut profiler: Box<dyn ServerProfiler + Send>,
    csv_profiling: bool,
    console_profiling: bool,
    mut transfer_result_receiver: UnboundedReceiver<TransferResult>,
    feedback_sender: Sender<ServerFeedbackMessage>,
    round_duration: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut round_stats = setup_round_stats(csv_profiling, console_profiling).unwrap();

        loop {
            let (transfer_result, total_time) = channel_pull(&mut transfer_result_receiver)
                .await
                .expect("Transfer result channel closed, terminating.");

            let mut frame_stats = transfer_result.frame_stats;
            frame_stats.total_time = total_time;

            round_stats.profile_frame(frame_stats);

            let current_round_duration = round_stats.start_time.elapsed();

            if current_round_duration.gt(&round_duration) {
                round_stats.log();
                round_stats.reset();
            }

            while let Some(message) = profiler.pull_feedback().await {
                feedback_sender.send(message)
                    .expect("Unable to broadcast a feedback message");
            }
        }
    })
}

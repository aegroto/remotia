use std::collections::HashMap;

use async_trait::async_trait;
use log::debug;
use rand::prelude::{SliceRandom, ThreadRng};

use crate::{
    server::pipeline::ascode::{feeder::AscodePipelineFeeder, AscodePipeline},
    traits::FrameProcessor,
    types::FrameData,
};

pub struct PoolingSwitch {
    entries: Vec<(u128, AscodePipelineFeeder)>
}

impl PoolingSwitch {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    pub fn entry(mut self, key: u128, pipeline: &AscodePipeline) -> Self {
        self.entries.push((key, pipeline.get_feeder()));
        self
    }
}

#[async_trait]
impl FrameProcessor for PoolingSwitch {
    async fn process(&mut self, mut frame_data: FrameData) -> Option<FrameData> {
        let (key, feeder) = self.entries.choose(&mut rand::thread_rng()).unwrap();

        debug!("Feeding to pipeline #{}...", key);

        frame_data.set("pool_key", *key);
        feeder.feed(frame_data);

        None
    }
}

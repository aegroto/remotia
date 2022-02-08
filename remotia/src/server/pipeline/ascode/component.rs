use std::time::Duration;

use log::{debug, info};
use tokio::{sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle, time::Interval};

use crate::server::{traits::FrameProcessor, types::ServerFrameData};

pub struct Component {
    processors: Vec<Box<dyn FrameProcessor + Send>>,

    receiver: Option<UnboundedReceiver<ServerFrameData>>,
    sender: Option<UnboundedSender<ServerFrameData>>,

    interval: Option<Interval>
}

unsafe impl Send for Component { }

impl Component {
    pub fn new() -> Self {
        Self {
            processors: Vec::new(),
            receiver: None,
            sender: None,
            interval: None
        }
    }

    pub fn set_sender(&mut self, sender: UnboundedSender<ServerFrameData>) {
        self.sender = Some(sender);
    }

    pub fn set_receiver(&mut self, receiver: UnboundedReceiver<ServerFrameData>) {
        self.receiver = Some(receiver);
    }

    pub fn with_tick(mut self, tick_interval: u64) -> Self {
        self.interval = Some(tokio::time::interval(Duration::from_millis(tick_interval)));
        self
    }

    pub fn add<T: 'static + FrameProcessor + Send>(mut self, processor: T) -> Self {
        self.processors.push(Box::new(processor));
        self
    }

    pub fn launch(mut self) -> JoinHandle<()> {
        tokio::spawn(async move {
            loop {
                if self.interval.is_some() {
                    self.interval.as_mut().unwrap().tick().await;
                }

                let mut frame_data = if self.receiver.is_some() {
                    self.receiver.as_mut().unwrap().recv().await.unwrap()
                } else {
                    ServerFrameData::default()
                };

                for processor in &mut self.processors {
                    frame_data = processor.process(frame_data).await;
                }

                if self.sender.is_some() {
                    self.sender.as_mut().unwrap().send(frame_data).unwrap();
                }
            }
        })
    }
}

use std::time::Duration;

use log::{debug, info};
use tokio::{sync::mpsc::{UnboundedSender, UnboundedReceiver}, task::JoinHandle, time::Interval};

use crate::{traits::FrameProcessor, types::FrameData};

pub struct Component {
    processors: Vec<Box<dyn FrameProcessor + Send>>,

    receiver: Option<UnboundedReceiver<FrameData>>,
    sender: Option<UnboundedSender<FrameData>>,

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

    pub fn set_sender(&mut self, sender: UnboundedSender<FrameData>) {
        self.sender = Some(sender);
    }

    pub fn set_receiver(&mut self, receiver: UnboundedReceiver<FrameData>) {
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
                    Some(self.receiver.as_mut().unwrap().recv().await.expect("Receive channel closed"))
                } else {
                    debug!("No receiver registered, allocating an empty frame DTO");
                    Some(FrameData::default())
                };

                debug!("Received frame data: {}", frame_data.as_ref().unwrap());

                for processor in &mut self.processors {
                    frame_data = processor.process(frame_data.unwrap()).await;

                    if frame_data.is_none() {
                        break;
                    }
                }

                if frame_data.is_some() && self.sender.is_some() {
                    debug!("Sending frame data: {}", frame_data.as_ref().unwrap());
                    if let Err(_) = self.sender.as_mut().unwrap().send(frame_data.unwrap()) {
                        panic!("Error while sending frame data");
                    }
                }
            }
        })
    }
}

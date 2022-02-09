use log::info;
use tokio::sync::mpsc;

use crate::types::FrameData;

use self::component::Component;

pub mod component;

pub struct AscodePipeline {
    components: Vec<Component> 
}

impl AscodePipeline {
    pub fn new() -> Self {
        Self {
            components: Vec::new()
        }
    }

    pub fn link(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    pub async fn run(mut self) {
        // Bind channels
        info!("Binding channels...");

        for i in 0..self.components.len()-1 {
            let (sender, receiver) = mpsc::unbounded_channel::<FrameData>();

            let src_component = self.components.get_mut(i).unwrap();
            src_component.set_sender(sender);

            let dst_component = self.components.get_mut(i + 1).unwrap();
            dst_component.set_receiver(receiver);
        }
        // TODO

        // Launch threads
        info!("Launching threads...");
        let mut handles = Vec::new();

        for component in self.components {
            let handle = component.launch();
            handles.push(handle);
        }

        for handle in handles {
            handle.await.unwrap();
        }
    }
}
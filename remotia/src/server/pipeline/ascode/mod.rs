use log::info;

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

    pub fn add(mut self, component: Component) -> Self {
        self.components.push(component);
        self
    }

    pub async fn run(self) {
        // Bind channels
        info!("Binding channels...");
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
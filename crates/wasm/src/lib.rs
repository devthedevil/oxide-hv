use oxide_engine::{Cluster, ClusterConfig};
use wasm_bindgen::prelude::*;

/// Opaque handle the JS/TS dashboard drives one `tick()` at a time,
/// pulling a JSON snapshot after each step to render.
#[wasm_bindgen]
pub struct SimHandle {
    cluster: Cluster,
}

#[wasm_bindgen]
impl SimHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> SimHandle {
        console_error_panic_hook::set_once();
        SimHandle {
            cluster: Cluster::new(ClusterConfig::default()),
        }
    }

    pub fn tick(&mut self) {
        self.cluster.tick();
    }

    pub fn snapshot(&self) -> Result<JsValue, JsValue> {
        serde_wasm_bindgen::to_value(&self.cluster.snapshot()).map_err(|e| JsValue::from_str(&e.to_string()))
    }

    pub fn fail_host(&mut self, host_id: u32) {
        self.cluster.fail_host(host_id);
    }

    pub fn recover_host(&mut self, host_id: u32) {
        self.cluster.recover_host(host_id);
    }

    pub fn inject_burst(&mut self, count: u32) {
        self.cluster.inject_burst(count);
    }
}

impl Default for SimHandle {
    fn default() -> Self {
        Self::new()
    }
}

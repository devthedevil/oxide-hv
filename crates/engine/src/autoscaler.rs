use serde::Serialize;

use crate::vm::{VmId, VmSpec};

#[derive(Debug, Clone, Copy)]
pub struct AutoscalerConfig {
    pub scale_out_cpu_threshold: f64,
    pub scale_in_cpu_threshold: f64,
    pub cooldown_ticks: u32,
    pub scale_out_batch: u32,
}

impl Default for AutoscalerConfig {
    fn default() -> Self {
        Self {
            scale_out_cpu_threshold: 0.75,
            scale_in_cpu_threshold: 0.25,
            cooldown_ticks: 5,
            scale_out_batch: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind")]
pub enum ScalingEvent {
    ScaleOut { count: u32 },
    ScaleIn { vm_id: VmId },
}

pub struct Autoscaler {
    pub config: AutoscalerConfig,
    cooldown_remaining: u32,
}

impl Autoscaler {
    pub fn new(config: AutoscalerConfig) -> Self {
        Self { config, cooldown_remaining: 0 }
    }

    /// Evaluates fleet-average CPU utilization against thresholds and
    /// returns a scaling decision, respecting a cooldown window so the
    /// controller doesn't thrash on transient spikes.
    pub fn evaluate(&mut self, avg_cpu_utilization: f64, idle_vm: Option<VmId>) -> Option<ScalingEvent> {
        if self.cooldown_remaining > 0 {
            self.cooldown_remaining -= 1;
            return None;
        }

        if avg_cpu_utilization >= self.config.scale_out_cpu_threshold {
            self.cooldown_remaining = self.config.cooldown_ticks;
            return Some(ScalingEvent::ScaleOut { count: self.config.scale_out_batch });
        }

        if avg_cpu_utilization <= self.config.scale_in_cpu_threshold {
            if let Some(vm_id) = idle_vm {
                self.cooldown_remaining = self.config.cooldown_ticks;
                return Some(ScalingEvent::ScaleIn { vm_id });
            }
        }

        None
    }
}

pub fn default_scale_out_spec() -> VmSpec {
    VmSpec::small()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scales_out_when_over_threshold() {
        let mut scaler = Autoscaler::new(AutoscalerConfig::default());
        let event = scaler.evaluate(0.9, None);
        assert!(matches!(event, Some(ScalingEvent::ScaleOut { .. })));
    }

    #[test]
    fn scales_in_only_when_idle_vm_available() {
        let mut scaler = Autoscaler::new(AutoscalerConfig::default());
        assert!(scaler.evaluate(0.1, None).is_none());

        let mut scaler = Autoscaler::new(AutoscalerConfig::default());
        let event = scaler.evaluate(0.1, Some(42));
        assert!(matches!(event, Some(ScalingEvent::ScaleIn { vm_id: 42 })));
    }

    #[test]
    fn respects_cooldown_between_events() {
        let mut scaler = Autoscaler::new(AutoscalerConfig {
            cooldown_ticks: 3,
            ..AutoscalerConfig::default()
        });
        assert!(scaler.evaluate(0.95, None).is_some());
        assert!(scaler.evaluate(0.95, None).is_none(), "should be in cooldown");
        assert!(scaler.evaluate(0.95, None).is_none());
        assert!(scaler.evaluate(0.95, None).is_none());
        assert!(scaler.evaluate(0.95, None).is_some(), "cooldown should have elapsed");
    }
}

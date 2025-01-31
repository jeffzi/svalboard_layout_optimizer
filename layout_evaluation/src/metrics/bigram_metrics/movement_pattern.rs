//! The bigram metric [`MovementPattern`] puts cost on each bigram that is mapped to
//! (almost) neighboring fingers. Which finger combinations come with which costs is
//! configurable.

use super::BigramMetric;

use keyboard_layout::{
    key::{Finger, Hand, HandFingerMap},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Copy, Clone, Deserialize, Debug)]
pub struct FingerSwitchCost {
    pub from: (Hand, Finger),
    pub to: (Hand, Finger),
    pub cost: f64,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Cost associated with bigrams from a finger to another one
    finger_switch_factor: Vec<FingerSwitchCost>,
}

#[derive(Clone, Debug)]
pub struct MovementPattern {
    finger_switch_factor: HandFingerMap<HandFingerMap<f64>>,
}

impl MovementPattern {
    pub fn new(params: &Parameters) -> Self {
        let mut finger_switch_factor =
            HandFingerMap::with_default(HandFingerMap::with_default(0.0));
        params.finger_switch_factor.iter().for_each(|fsc| {
            let m = finger_switch_factor.get_mut(&fsc.from.0, &fsc.from.1);
            m.set(&fsc.to.0, &fsc.to.1, fsc.cost);
        });

        Self {
            finger_switch_factor,
        }
    }
}

impl BigramMetric for MovementPattern {
    fn name(&self) -> &str {
        "Movement Pattern"
    }

    #[inline(always)]
    fn individual_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        weight: f64,
        _total_weight: f64,
        _layout: &Layout,
    ) -> Option<f64> {
        let f1 = k1.key.finger;
        let f2 = k2.key.finger;
        let h1 = k1.key.hand;
        let h2 = k2.key.hand;

        if f1 == Finger::Thumb || f2 == Finger::Thumb || h1 != h2 || f1 == f2 {
            return Some(0.0);
        }

        let cost = self.finger_switch_factor.get(&h1, &f1).get(&h2, &f2);

        Some(weight * cost)
    }
}

//! This metric takes into account how an intra-cluster roll feels, because (at
//! least for me):
//! - center -> south and pad -> up feel *great*
//! - center -> (in|out) feels decent
//! - a bunch of the other ones are *terrible*

use super::BigramMetric;

use ahash::AHashMap;
use keyboard_layout::{
    key::{Direction, Finger},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub default_cost: f64,
    pub costs: AHashMap<Direction, AHashMap<Direction, f64>>,
    pub finger_multipliers: AHashMap<Finger, f64>,
}

#[derive(Clone, Debug)]
pub struct ClusterRolls {
    default_cost: f64,
    costs: AHashMap<Direction, AHashMap<Direction, f64>>,
    finger_multipliers: AHashMap<Finger, f64>,
}

impl ClusterRolls {
    pub fn new(params: &Parameters) -> Self {
        Self {
            costs: params.costs.clone(),
            default_cost: params.default_cost,
            finger_multipliers: params.finger_multipliers.clone(),
        }
    }
}

impl BigramMetric for ClusterRolls {
    fn name(&self) -> &str {
        "Cluster Rolls"
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
        if (k1 == k2 && k1.is_modifier.is_some())
            || k1.key.hand != k2.key.hand
            || k1.key.finger != k2.key.finger
        {
            return Some(0.0);
        }

        let finger = k1.key.finger; // same for k1, k2
        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        let base_cost = match self.costs.get(&dir_from) {
            Some(m) => match m.get(&dir_to) {
                Some(base_cost) => *base_cost,
                _ => self.default_cost,
            }
            _ => self.default_cost,
        };

        let cost = weight * base_cost * (match self.finger_multipliers.get(&finger) {
            Some(m) => *m,
            _ => 1.0,
        });

        Some(cost)
    }
}

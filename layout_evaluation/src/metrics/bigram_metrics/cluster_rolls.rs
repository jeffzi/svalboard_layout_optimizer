//! This metric takes into account how an intra-cluster roll feels, because (at
//! least for me):
//! - center -> south and pad -> up feel *great*
//! - center -> (in|out) feels decent
//! - a bunch of the other ones are *terrible*

use super::BigramMetric;

use ahash::AHashMap;
use keyboard_layout::{
    key::Direction,
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub costs: AHashMap<Direction, AHashMap<Direction, f64>>,
    pub default_cost: f64,
}

#[derive(Clone, Debug)]
pub struct ClusterRolls {
    costs: AHashMap<Direction, AHashMap<Direction, f64>>,
    default_cost: f64,
}

impl ClusterRolls {
    pub fn new(params: &Parameters) -> Self {
        Self {
            costs: params.costs.clone(),
            default_cost: params.default_cost,
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

        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        let base_cost = match self.costs.get(&dir_from) {
            Some(m) => match m.get(&dir_to) {
                Some(cost) => *cost,
                _ => self.default_cost,
            }
            _ => self.default_cost,
        };

        Some(weight * base_cost)
    }
}

//! This metric takes into account how an intra-cluster roll feels, because (at
//! least for me):
//! - center -> south and pad -> up feel *great*
//! - center -> (in|out) feels decent
//! - a bunch of the other ones are *terrible*

use super::BigramMetric;

use keyboard_layout::{
    key::{Direction::*, Finger},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub south_north_cost: f64,
    pub lateral_squeeze_cost: f64,
    pub lateral_splay_cost: f64,
    pub lateral_series_cost: f64,
    pub lateral_center_cost: f64,
}

#[derive(Clone, Debug)]
pub struct Scissoring {
    south_north_cost: f64,
    lateral_squeeze_cost: f64,
    lateral_splay_cost: f64,
    lateral_series_cost: f64,
    lateral_center_cost: f64,
}

impl Scissoring {
    pub fn new(params: &Parameters) -> Self {
        Self {
            south_north_cost: params.south_north_cost,
            lateral_squeeze_cost: params.lateral_squeeze_cost,
            lateral_splay_cost: params.lateral_splay_cost,
            lateral_series_cost: params.lateral_series_cost,
            lateral_center_cost: params.lateral_center_cost,
        }
    }
}

impl BigramMetric for Scissoring {
    fn name(&self) -> &str {
        "Scissoring"
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
        // only adjacent non-thumb fingers, please
        if (k1 == k2 && k1.is_modifier.is_some())
            || k1.key.hand != k2.key.hand
            || k1.key.finger.distance(&k2.key.finger) != 1
            || k1.key.finger == Finger::Thumb
            || k2.key.finger == Finger::Thumb
        {
            return Some(0.0);
        }

        let finger_from = k1.key.finger;
        let finger_to = k2.key.finger;

        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        let base_cost = match (dir_from, dir_to) {
            (South, North) | (North, South) => self.south_north_cost,
            (In, In) | (Out, Out) => self.lateral_series_cost,
            (In, Center) | (Out, Center) | (Center, In) | (Center, Out) => self.lateral_center_cost,
            (In, Out) | (Out, In) => {
                let inward_motion: bool = finger_from.numeric_index() > finger_to.numeric_index();

                // think about it for a sec
                let is_squeeze: bool = inward_motion ^ (dir_from == Out);

                if is_squeeze {
                    self.lateral_squeeze_cost
                } else {
                    self.lateral_splay_cost
                }
            }
            _ => 0.0
        };

        Some(base_cost * weight)
    }
}

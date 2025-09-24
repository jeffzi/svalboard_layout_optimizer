use super::BigramMetric;

use keyboard_layout::{
    key::{Finger, Hand},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub ignore_modifiers: bool,
    pub ignore_thumbs: bool,
}

#[derive(Clone, Debug)]
pub struct RollStats {
    ignore_modifiers: bool,
    ignore_thumbs: bool,
}

impl RollStats {
    pub fn new(params: &Parameters) -> Self {
        Self {
            ignore_modifiers: params.ignore_modifiers,
            ignore_thumbs: params.ignore_thumbs,
        }
    }

    fn should_ignore_key(&self, key: &LayerKey) -> bool {
        (self.ignore_thumbs && key.key.finger == Finger::Thumb)
            || (self.ignore_modifiers && key.is_modifier.is_some())
    }

    fn is_inward_roll(&self, k1: &LayerKey, k2: &LayerKey) -> bool {
        // Same hand, different fingers
        if k1.key.hand != k2.key.hand || k1.key.finger == k2.key.finger {
            return false;
        }

        // Check if it's an inward roll (towards index finger)
        match k1.key.hand {
            Hand::Left => {
                // Left hand: inward means lower matrix position to higher (pinky->ring->middle->index)
                k1.key.matrix_position.0 < k2.key.matrix_position.0
            }
            Hand::Right => {
                // Right hand: inward means higher matrix position to lower (pinky->ring->middle->index)
                k1.key.matrix_position.0 > k2.key.matrix_position.0
            }
        }
    }

    fn is_outward_roll(&self, k1: &LayerKey, k2: &LayerKey) -> bool {
        // Same hand, different fingers
        if k1.key.hand != k2.key.hand || k1.key.finger == k2.key.finger {
            return false;
        }

        // Check if it's an outward roll (towards pinky)
        match k1.key.hand {
            Hand::Left => {
                // Left hand: outward means higher matrix position to lower (index->middle->ring->pinky)
                k1.key.matrix_position.0 > k2.key.matrix_position.0
            }
            Hand::Right => {
                // Right hand: outward means lower matrix position to higher (index->middle->ring->pinky)
                k1.key.matrix_position.0 < k2.key.matrix_position.0
            }
        }
    }

    fn is_center_south_roll(&self, k1: &LayerKey, k2: &LayerKey) -> bool {
        // Same finger, center to south movement
        k1.key.finger == k2.key.finger
            && k1.key.matrix_position.0 == k2.key.matrix_position.0
            && k1.key.matrix_position.1 == 2 // center row
            && k2.key.matrix_position.1 == 3 // south row
    }
}

impl BigramMetric for RollStats {
    fn name(&self) -> &str {
        "Roll Statistics"
    }

    fn total_cost(
        &self,
        bigrams: &[((&LayerKey, &LayerKey), f64)],
        total_weight: Option<f64>,
        _layout: &Layout,
    ) -> (f64, Option<String>) {
        let _total_weight = total_weight.unwrap_or_else(|| bigrams.iter().map(|(_, w)| w).sum());

        let mut inward_rolls_weight = 0.0;
        let mut outward_rolls_weight = 0.0;
        let mut center_south_rolls_weight = 0.0;
        let mut valid_bigrams_weight = 0.0;

        for ((k1, k2), weight) in bigrams {
            // Skip ignored keys
            if self.should_ignore_key(k1) || self.should_ignore_key(k2) {
                continue;
            }

            valid_bigrams_weight += weight;

            if self.is_inward_roll(k1, k2) {
                inward_rolls_weight += weight;
            } else if self.is_outward_roll(k1, k2) {
                outward_rolls_weight += weight;
            } else if self.is_center_south_roll(k1, k2) {
                center_south_rolls_weight += weight;
            }
        }

        let inward_percentage = if valid_bigrams_weight > 0.0 {
            (inward_rolls_weight / valid_bigrams_weight) * 100.0
        } else {
            0.0
        };

        let outward_percentage = if valid_bigrams_weight > 0.0 {
            (outward_rolls_weight / valid_bigrams_weight) * 100.0
        } else {
            0.0
        };

        let center_south_percentage = if valid_bigrams_weight > 0.0 {
            (center_south_rolls_weight / valid_bigrams_weight) * 100.0
        } else {
            0.0
        };

        let total_rolls_percentage = inward_percentage + outward_percentage + center_south_percentage;

        let message = format!(
            "Inward: {:.1}%, Outward: {:.1}%, Center->South: {:.1}%, Total Rolls: {:.1}%",
            inward_percentage, outward_percentage, center_south_percentage, total_rolls_percentage
        );

        // Return 0 cost since this is informational only
        (0.0, Some(message))
    }
}
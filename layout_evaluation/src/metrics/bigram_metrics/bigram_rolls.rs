use super::BigramMetric;
use crate::metrics::format_utils::should_show_ngram;

use keyboard_layout::{
    key::{Direction, Hand},
    layout::{LayerKey, Layout},
};

use ordered_float::OrderedFloat;
use priority_queue::DoublePriorityQueue;
use serde::Deserialize;
use std::env;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub ignore_thumbs: bool,
    pub ignore_modifiers: bool,
    /// Weight multiplier for inward rolls (toward index finger)
    pub inward_factor: f64,
    /// Weight multiplier for outward rolls (toward pinky)
    pub outward_factor: f64,
    /// List of same-finger movements to track as rolls (e.g., [[Center, South]])
    /// If empty, same-finger movements are excluded from rolls
    #[serde(default = "default_same_finger_movements")]
    pub same_finger_movements: Vec<(Direction, Direction)>,
}

fn default_same_finger_movements() -> Vec<(Direction, Direction)> {
    vec![]
}

#[derive(Clone, Debug)]
pub struct BigramRolls {
    ignore_thumbs: bool,
    ignore_modifiers: bool,
    inward_factor: f64,
    outward_factor: f64,
    same_finger_movements: Vec<(Direction, Direction)>,
}

impl BigramRolls {
    pub fn new(params: &Parameters) -> Self {
        Self {
            ignore_thumbs: params.ignore_thumbs,
            ignore_modifiers: params.ignore_modifiers,
            inward_factor: params.inward_factor,
            outward_factor: params.outward_factor,
            same_finger_movements: params.same_finger_movements.clone(),
        }
    }

    fn should_ignore_key(&self, key: &LayerKey) -> bool {
        use keyboard_layout::key::Finger;
        (self.ignore_thumbs && key.key.finger == Finger::Thumb)
            || (self.ignore_modifiers && key.is_modifier.is_some())
    }

    /// Check if a same-finger movement matches the configured same_finger_movements
    fn is_tracked_same_finger_movement(&self, k1: &LayerKey, k2: &LayerKey) -> bool {
        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        self.same_finger_movements
            .iter()
            .any(|&(from, to)| dir_from == from && dir_to == to)
    }
}

impl BigramMetric for BigramRolls {
    fn name(&self) -> &str {
        "Bigram Rolls"
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
        // Skip if any key should be ignored
        if self.should_ignore_key(k1) || self.should_ignore_key(k2) {
            return Some(0.0);
        }

        // Must be on same hand for a roll (check this FIRST before finger checks)
        if k1.key.hand != k2.key.hand {
            return Some(0.0);
        }

        // Check if same finger (now we know they're on the same hand)
        if k1.key.finger == k2.key.finger {
            // Only count if this movement is in same_finger_movements list
            if self.is_tracked_same_finger_movement(k1, k2) {
                // For same-finger movements, we don't distinguish inward/outward
                // Use the average of the two factors
                let factor = (self.inward_factor + self.outward_factor) / 2.0;
                // Return negative cost (reward) - will be multiplied by positive weight
                return Some(-weight * factor);
            } else {
                return Some(0.0);
            }
        }

        log::trace!(
            "Bigram {} {} - same hand roll: {:?}",
            k1.symbol,
            k2.symbol,
            k1.key.hand
        );

        // Different fingers, same hand: classify as inward or outward roll
        let inwards = if k1.key.hand == Hand::Left {
            k1.key.matrix_position.0 < k2.key.matrix_position.0
        } else {
            k1.key.matrix_position.0 > k2.key.matrix_position.0
        };

        let factor = if inwards {
            self.inward_factor
        } else {
            self.outward_factor
        };

        // Return negative cost (reward) - will be multiplied by positive weight
        Some(-weight * factor)
    }

    fn total_cost(
        &self,
        bigrams: &[((&LayerKey, &LayerKey), f64)],
        total_weight: Option<f64>,
        layout: &Layout,
    ) -> (f64, Option<String>) {
        let show_worst: bool = env::var("SHOW_WORST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(true);
        let n_worst: usize = env::var("N_WORST")
            .ok()
            .and_then(|s| s.parse().ok())
            .unwrap_or(3);

        let total_weight = total_weight.unwrap_or_else(|| bigrams.iter().map(|(_, w)| w).sum());

        let cost_iter = bigrams
            .iter()
            .enumerate()
            .filter_map(|(i, (bigram, weight))| {
                let cost_option =
                    self.individual_cost(bigram.0, bigram.1, *weight, total_weight, layout);
                cost_option.map(|cost| (i, bigram, cost))
            });

        let (total_cost, msg) = if show_worst {
            // For rolls, we want to show the BEST (most negative/rewarding) bigrams
            let (total_cost, best_rolls) = cost_iter.fold(
                (0.0, DoublePriorityQueue::new()),
                |(mut total_cost, mut best_rolls), (i, _bigram, cost)| {
                    total_cost += cost;

                    // Only track negative costs (rolls)
                    if cost < 0.0 {
                        // Use negative of cost so most negative becomes highest priority
                        best_rolls.push(i, OrderedFloat(-cost));

                        if best_rolls.len() > n_worst {
                            best_rolls.pop_min();
                        }
                    }

                    (total_cost, best_rolls)
                },
            );

            let roll_msgs: Vec<String> = best_rolls
                .into_sorted_iter()
                .rev()
                .map(|(i, neg_cost)| {
                    let (gram, weight) = bigrams[i];
                    let freq_pct = 100.0 * weight / total_weight;
                    let cost = -neg_cost.into_inner();
                    let reward_pct = 100.0 * cost.abs() / total_cost.abs();
                    (i, gram, reward_pct, freq_pct)
                })
                .filter(|(_, _, reward_pct, freq_pct)| {
                    should_show_ngram(*reward_pct, Some(*freq_pct))
                })
                .map(|(_, gram, reward_pct, freq_pct)| {
                    let bigram_str = format!("{}{}", gram.0, gram.1);
                    format!("{} ({:>5.2}%|{:>5.2}%)", bigram_str, reward_pct, freq_pct)
                })
                .collect();

            let msg = if !roll_msgs.is_empty() {
                Some(format!("Best rolls: {}", roll_msgs.join(", ")))
            } else {
                None
            };

            (total_cost, msg)
        } else {
            let total_cost: f64 = cost_iter.map(|(_, _, c)| c).sum();
            (total_cost, None)
        };

        (total_cost, msg)
    }
}

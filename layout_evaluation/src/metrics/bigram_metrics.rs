//! The `metrics` module provides a trait for bigram metrics.
use keyboard_layout::{
    key::{Direction, Finger},
    layout::{LayerKey, Layout},
};

use super::format_utils::{format_percentages, should_show_ngram, visualize_whitespace};
use ordered_float::OrderedFloat;
use priority_queue::DoublePriorityQueue;
use std::{env, fmt};

/// Check if two keys represent adjacent non-thumb fingers on the same hand
///
/// Returns true if:
/// - Not the same key with a modifier
/// - Both on the same hand
/// - Adjacent fingers (distance of 1)
/// - Neither is a thumb
#[inline]
pub fn is_adjacent_fingers(k1: &LayerKey, k2: &LayerKey) -> bool {
    !((k1 == k2 && k1.is_modifier.is_some())
        || k1.key.hand != k2.key.hand
        || k1.key.finger.distance(&k2.key.finger) != 1
        || k1.key.finger == Finger::Thumb
        || k2.key.finger == Finger::Thumb)
}

/// Returns true if adjacent fingers move in different directions.
/// Same-direction movements benefit from finger coupling (enslaving helps),
/// while different directions create conflict (enslaving fights the second finger).
/// Center (rest position) is ignored - not a movement.
#[inline]
pub fn has_coupling_conflict(dir1: Direction, dir2: Direction) -> bool {
    dir1 != dir2 && dir1 != Direction::Center && dir2 != Direction::Center
}

pub mod bigram_rolls;
pub mod bigram_stats;
pub mod finger_repeats;
pub mod fsb;
pub mod hsb;
pub mod kla_distance;
pub mod kla_finger_usage;
pub mod kla_same_finger;
pub mod kla_same_hand;
pub mod manual_bigram_penalty;
pub mod movement_pattern;
pub mod no_handswitch_after_unbalancing_key;
pub mod oxey_lsbs;
pub mod oxey_sfbs;
mod scissor_base;
pub mod sfb;
pub mod sympathetic;
pub mod symmetric_handswitches;

/// BigramMetric is a trait for metrics that iterates over weighted bigrams.
pub trait BigramMetric: Send + Sync + BigramMetricClone + fmt::Debug {
    /// Return the name of the metric.
    fn name(&self) -> &str;

    /// Compute the cost of one bigram (if that is possible, otherwise, return `None`).
    #[inline(always)]
    fn individual_cost(
        &self,
        _key1: &LayerKey,
        _key2: &LayerKey,
        _weight: f64,
        _total_weight: f64,
        _layout: &Layout,
    ) -> Option<f64> {
        None
    }

    /// Compute the total cost for the metric.
    fn total_cost(
        &self,
        bigrams: &[((&LayerKey, &LayerKey), f64)],
        // total_weight is optional for performance reasons (it can be computed from bigrams).
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
            let (total_cost, worst) = cost_iter.fold(
                (0.0, DoublePriorityQueue::new()),
                |(mut total_cost, mut worst), (i, _bigram, cost)| {
                    total_cost += cost;

                    worst.push(i, OrderedFloat(cost));

                    if worst.len() > n_worst {
                        worst.pop_min();
                    }

                    (total_cost, worst)
                },
            );

            let worst_msgs: Vec<String> = worst
                .into_sorted_iter()
                .rev()
                .filter(|(_, cost)| cost.into_inner() > 0.0)
                .map(|(i, cost)| {
                    let (gram, weight) = bigrams[i];
                    let freq_pct = 100.0 * weight / total_weight;
                    let cost_pct = 100.0 * cost.into_inner() / total_cost;
                    (i, gram, cost_pct, freq_pct)
                })
                .filter(|(_, _, cost_pct, freq_pct)| should_show_ngram(*cost_pct, Some(*freq_pct)))
                .map(|(_, gram, cost_pct, freq_pct)| {
                    let percentages = format_percentages(cost_pct, freq_pct);
                    let bigram_str = format!("{}{}", gram.0, gram.1);
                    format!("{} {}", visualize_whitespace(&bigram_str), percentages)
                })
                .collect();

            let msg = if !worst_msgs.is_empty() {
                Some(worst_msgs.join(", "))
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

impl Clone for Box<dyn BigramMetric> {
    fn clone(&self) -> Box<dyn BigramMetric> {
        self.clone_box()
    }
}

/// Helper trait for realizing clonability for `Box<dyn BigramMetric>`.
pub trait BigramMetricClone {
    fn clone_box(&self) -> Box<dyn BigramMetric>;
}

impl<T> BigramMetricClone for T
where
    T: 'static + BigramMetric + Clone,
{
    fn clone_box(&self) -> Box<dyn BigramMetric> {
        Box::new(self.clone())
    }
}

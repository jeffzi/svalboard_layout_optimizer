//! Base implementation for scissor-type metrics (FSB and HSB)
//!
//! This module provides generic infrastructure for scissor metrics that:
//! - Track worst bigrams by category (e.g., Vertical, Squeeze, Diagonal)
//! - Apply optional frequency-based multipliers for critical bigrams
//! - Apply optional finger-specific multipliers
//! - Format output with consistent whitespace visualization and percentage display
use super::BigramMetric;
use crate::metrics::format_utils::{format_percentages, should_show_ngram, visualize_whitespace};
use ahash::AHashMap;
use keyboard_layout::{
    key::Finger,
    layout::{LayerKey, Layout},
};
use ordered_float::OrderedFloat;
use priority_queue::DoublePriorityQueue;
use std::{collections::HashMap, env, fmt::Debug, hash::Hash};

/// Trait for scissor metric categories (Vertical, Squeeze, Diagonal, etc.)
pub trait ScissorCategory: Clone + Debug + PartialEq + Eq + Hash + Send + Sync {
    /// Get all categories in display order
    fn display_order() -> &'static [Self];

    /// Get the display name for this category
    fn display_name(&self) -> String;
}

/// Trait for computing scissor costs
pub trait ScissorCompute<C: ScissorCategory>: Clone + Debug + Send + Sync {
    fn compute_cost(&self, k1: &LayerKey, k2: &LayerKey, layout: &Layout) -> Option<(f64, C)>;
}

/// Check if two keys represent adjacent non-thumb fingers on the same hand
///
/// Returns true if:
/// - Not the same key with a modifier
/// - Both on the same hand
/// - Adjacent fingers (distance of 1)
/// - Neither is a thumb
#[inline]
pub fn is_adjacent_fingers(k1: &LayerKey, k2: &LayerKey) -> bool {
    use keyboard_layout::key::Finger;

    !((k1 == k2 && k1.is_modifier.is_some())
        || k1.key.hand != k2.key.hand
        || k1.key.finger.distance(&k2.key.finger) != 1
        || k1.key.finger == Finger::Thumb
        || k2.key.finger == Finger::Thumb)
}

/// Classification of scissor movement types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScissorType {
    /// Full Scissor Vertical - North-South opposition
    Vertical,
    /// Full Scissor Squeeze - Fingers moving toward each other
    Squeeze,
    /// Full Scissor Splay - Fingers moving apart
    Splay,
    /// Half Scissor - Diagonal movements (lateral + vertical)
    Diagonal,
    /// Lateral - Lateral displacement with center
    Lateral,
}

/// Classify a bigram as a scissor movement type
///
/// Returns `Some(ScissorType)` if the bigram represents a scissor movement,
/// or `None` if it's not a scissor (e.g., rolling motion, different hands, etc.)
#[inline]
pub fn classify_scissor(k1: &LayerKey, k2: &LayerKey) -> Option<ScissorType> {
    use keyboard_layout::key::{Direction::*, Finger};

    // Only adjacent non-thumb fingers
    if k1.key.hand != k2.key.hand
        || k1.key.finger.distance(&k2.key.finger) != 1
        || k1.key.finger == Finger::Thumb
        || k2.key.finger == Finger::Thumb
    {
        return None;
    }

    let finger_from = k1.key.finger;
    let finger_to = k2.key.finger;
    let dir_from = k1.key.direction;
    let dir_to = k2.key.direction;

    match (dir_from, dir_to) {
        // NOT a scissor: just rolling (same lateral direction)
        (In, In) | (Out, Out) => None,

        // Full Scissor Vertical - North-South opposition
        (South, North) | (North, South) => Some(ScissorType::Vertical),

        // Full Scissor Lateral - In-Out opposition (squeeze/splay)
        (In, Out) | (Out, In) => {
            let inward_motion = finger_from.numeric_index() > finger_to.numeric_index();
            let is_squeeze = inward_motion ^ (dir_from == Out);

            if is_squeeze {
                Some(ScissorType::Squeeze)
            } else {
                Some(ScissorType::Splay)
            }
        }

        // Half Scissor - Diagonal movements (lateral + vertical)
        (In, North)
        | (Out, North)
        | (North, In)
        | (North, Out)
        | (In, South)
        | (Out, South)
        | (South, In)
        | (South, Out) => Some(ScissorType::Diagonal),

        // Lateral - Lateral displacement with center
        (In, Center) | (Out, Center) | (Center, In) | (Center, Out) => Some(ScissorType::Lateral),

        // All other combinations: not considered scissors
        _ => None,
    }
}

/// Generic scissor metric implementation
#[derive(Clone, Debug)]
pub struct ScissorMetric<C: ScissorCategory, T: ScissorCompute<C>> {
    name: &'static str,
    critical_bigram_fraction: Option<f64>,
    critical_bigram_factor: Option<f64>,
    finger_factors: Option<AHashMap<Finger, f64>>,
    compute: T,
    _phantom: std::marker::PhantomData<C>,
}

impl<C: ScissorCategory, T: ScissorCompute<C>> ScissorMetric<C, T> {
    pub fn new(
        name: &'static str,
        critical_bigram_fraction: Option<f64>,
        critical_bigram_factor: Option<f64>,
        finger_factors: Option<AHashMap<Finger, f64>>,
        compute: T,
    ) -> Self {
        Self {
            name,
            critical_bigram_fraction,
            critical_bigram_factor,
            finger_factors,
            compute,
            _phantom: std::marker::PhantomData,
        }
    }

    /// Calculate frequency multiplier for critical bigrams
    #[inline]
    fn frequency_multiplier(&self, weight: f64, total_weight: f64) -> f64 {
        if let (Some(threshold), Some(factor)) =
            (self.critical_bigram_fraction, self.critical_bigram_factor)
        {
            let relative_weight = weight / total_weight;
            if relative_weight > threshold {
                factor
            } else {
                1.0
            }
        } else {
            1.0
        }
    }

    /// Calculate finger multiplier based on both fingers involved
    /// Uses the maximum factor since the weaker finger dominates comfort
    #[inline]
    fn finger_multiplier(&self, k1: &LayerKey, k2: &LayerKey) -> f64 {
        if let Some(ref factors) = self.finger_factors {
            let factor1 = factors.get(&k1.key.finger).copied().unwrap_or(1.0);
            let factor2 = factors.get(&k2.key.finger).copied().unwrap_or(1.0);
            factor1.max(factor2)
        } else {
            1.0
        }
    }

    fn bigram_cost_with_category(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        layout: &Layout,
    ) -> Option<(f64, C)> {
        self.compute.compute_cost(k1, k2, layout)
    }

    fn bigram_cost(&self, k1: &LayerKey, k2: &LayerKey, layout: &Layout) -> Option<f64> {
        self.bigram_cost_with_category(k1, k2, layout)
            .map(|(cost, _)| cost)
    }
}

impl<C: ScissorCategory + 'static, T: ScissorCompute<C> + 'static> BigramMetric
    for ScissorMetric<C, T>
{
    fn name(&self) -> &str {
        self.name
    }

    #[inline(always)]
    fn individual_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        weight: f64,
        total_weight: f64,
        layout: &Layout,
    ) -> Option<f64> {
        match self.bigram_cost(k1, k2, layout) {
            Some(base_cost) => {
                let frequency_multiplier = self.frequency_multiplier(weight, total_weight);
                let finger_multiplier = self.finger_multiplier(k1, k2);
                Some(weight * base_cost * finger_multiplier * frequency_multiplier)
            }
            None => Some(0.0),
        }
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

        if !show_worst {
            let total_cost: f64 = bigrams
                .iter()
                .filter_map(|(bigram, weight)| {
                    self.individual_cost(bigram.0, bigram.1, *weight, total_weight, layout)
                })
                .sum();
            return (total_cost, None);
        }

        // Track worst bigrams by category
        let mut category_queues: HashMap<C, DoublePriorityQueue<usize, OrderedFloat<f64>>> =
            HashMap::new();
        let mut total_cost = 0.0;

        for (i, (bigram, weight)) in bigrams.iter().enumerate() {
            if let Some((base_cost, category)) =
                self.bigram_cost_with_category(bigram.0, bigram.1, layout)
            {
                let frequency_multiplier = self.frequency_multiplier(*weight, total_weight);
                let cost = weight * base_cost * frequency_multiplier;
                total_cost += cost;

                let queue = category_queues.entry(category).or_default();
                queue.push(i, OrderedFloat(cost));

                if queue.len() > n_worst {
                    queue.pop_min();
                }
            }
        }

        let mut category_msgs: Vec<String> = Vec::new();

        for category in C::display_order() {
            if let Some(queue) = category_queues.get(category) {
                let worst_msgs: Vec<String> = queue
                    .clone()
                    .into_sorted_iter()
                    .rev()
                    .filter(|(_, cost)| cost.into_inner() > 0.0)
                    .map(|(i, cost)| {
                        let (gram, weight) = bigrams[i];
                        let freq_pct = 100.0 * weight / total_weight;
                        let cost_pct = 100.0 * cost.into_inner() / total_cost;
                        (i, gram, cost_pct, freq_pct)
                    })
                    .filter(|(_, _, cost_pct, freq_pct)| {
                        should_show_ngram(*cost_pct, Some(*freq_pct))
                    })
                    .map(|(_, gram, cost_pct, freq_pct)| {
                        let percentages = format_percentages(cost_pct, freq_pct);
                        let bigram_str = format!("{}{}", gram.0, gram.1);
                        format!("{} {}", visualize_whitespace(&bigram_str), percentages)
                    })
                    .collect();

                if !worst_msgs.is_empty() {
                    category_msgs.push(format!(
                        "{}: {}",
                        category.display_name(),
                        worst_msgs.join(", ")
                    ));
                }
            }
        }

        let msg = if category_msgs.is_empty() {
            None
        } else {
            Some(category_msgs.join("; "))
        };

        (total_cost, msg)
    }
}

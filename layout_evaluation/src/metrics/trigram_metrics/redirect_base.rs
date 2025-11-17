//! Base implementation for redirect-type metrics
//!
//! This module provides generic infrastructure for redirect metrics that:
//! - Classify redirects (one-handed trigrams with direction changes)
//! - Distinguish between normal redirects (with index/thumb) and weak redirects (without)
//! - Format output with consistent whitespace visualization and percentage display

use super::TrigramMetric;
use crate::metrics::format_utils::{format_percentages, visualize_whitespace};
use keyboard_layout::{
    key::{Finger, Hand},
    layout::{LayerKey, Layout},
};
use ordered_float::OrderedFloat;
use priority_queue::DoublePriorityQueue;
use std::{env, fmt::Debug};

#[inline(always)]
fn inwards(k1: &LayerKey, k2: &LayerKey) -> bool {
    if k1.key.hand == Hand::Left {
        k1.key.matrix_position.0 < k2.key.matrix_position.0
    } else {
        k1.key.matrix_position.0 > k2.key.matrix_position.0
    }
}

/// Check if a trigram is a redirect and whether it's weak
/// Returns: (is_redirect, is_weak_redirect)
pub fn classify_redirect(k1: &LayerKey, k2: &LayerKey, k3: &LayerKey) -> (bool, bool) {
    let h1 = k1.key.hand;
    let h2 = k2.key.hand;
    let h3 = k3.key.hand;

    // Must be same hand (one-handed trigram)
    if !(h1 == h2 && h2 == h3) {
        return (false, false);
    }

    let f1 = k1.key.finger;
    let f2 = k2.key.finger;
    let f3 = k3.key.finger;

    // Must use different fingers (no same-finger bigrams)
    if f1 == f2 || f2 == f3 {
        return (false, false);
    }

    let inwards1 = inwards(k1, k2);
    let inwards2 = inwards(k2, k3);

    let outwards1 = inwards(k2, k1);
    let outwards2 = inwards(k3, k2);

    // Check for direction change: inward->outward or outward->inward
    let is_redirect = (inwards1 && outwards2) || (outwards1 && inwards2);

    if !is_redirect {
        return (false, false);
    }

    // Check if it's weak (no index finger or thumb)
    let has_index_or_thumb = f1 == Finger::Index
        || f2 == Finger::Index
        || f3 == Finger::Index
        || f1 == Finger::Thumb
        || f2 == Finger::Thumb
        || f3 == Finger::Thumb;
    let is_weak = !has_index_or_thumb;

    (true, is_weak)
}

/// Trait for filtering redirects based on weakness
pub trait RedirectFilter: Clone + Debug + Send + Sync {
    /// Returns true if this redirect should be counted by this metric
    fn should_count(&self, is_weak: bool) -> bool;
}

/// Filter for normal redirects (involving index/thumb)
#[derive(Clone, Debug)]
pub struct NormalRedirectFilter;

impl RedirectFilter for NormalRedirectFilter {
    fn should_count(&self, is_weak: bool) -> bool {
        !is_weak // Only count non-weak redirects
    }
}

/// Filter for weak redirects (not involving index/thumb)
#[derive(Clone, Debug)]
pub struct WeakRedirectFilter;

impl RedirectFilter for WeakRedirectFilter {
    fn should_count(&self, is_weak: bool) -> bool {
        is_weak // Only count weak redirects
    }
}

/// Generic redirect metric implementation
#[derive(Clone, Debug)]
pub struct RedirectMetric<F: RedirectFilter> {
    name: &'static str,
    filter: F,
    base_cost: f64,
    ignore_thumbs: bool,
    ignore_modifiers: bool,
}

impl<F: RedirectFilter> RedirectMetric<F> {
    pub fn new(
        name: &'static str,
        filter: F,
        base_cost: f64,
        ignore_thumbs: bool,
        ignore_modifiers: bool,
    ) -> Self {
        Self {
            name,
            filter,
            base_cost,
            ignore_thumbs,
            ignore_modifiers,
        }
    }

    fn should_ignore_key(&self, key: &LayerKey) -> bool {
        (self.ignore_thumbs && key.key.finger == Finger::Thumb)
            || (self.ignore_modifiers && key.is_modifier.is_some())
    }
}

impl<F: RedirectFilter + 'static> TrigramMetric for RedirectMetric<F> {
    fn name(&self) -> &str {
        self.name
    }

    #[inline(always)]
    fn individual_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        k3: &LayerKey,
        weight: f64,
        _total_weight: f64,
        _layout: &Layout,
    ) -> Option<f64> {
        // Skip if any key should be ignored
        if self.should_ignore_key(k1) || self.should_ignore_key(k2) || self.should_ignore_key(k3) {
            return Some(0.0);
        }

        let (is_redirect, is_weak) = classify_redirect(k1, k2, k3);

        if !is_redirect || !self.filter.should_count(is_weak) {
            return Some(0.0);
        }

        Some(weight * self.base_cost)
    }

    fn total_cost(
        &self,
        trigrams: &[((&LayerKey, &LayerKey, &LayerKey), f64)],
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

        let total_weight = total_weight.unwrap_or_else(|| trigrams.iter().map(|(_, w)| w).sum());

        if !show_worst {
            let total_cost: f64 = trigrams
                .iter()
                .filter_map(|(trigram, weight)| {
                    self.individual_cost(
                        trigram.0,
                        trigram.1,
                        trigram.2,
                        *weight,
                        total_weight,
                        layout,
                    )
                })
                .sum();
            return (total_cost, None);
        }

        // Track worst redirects
        let mut worst_queue: DoublePriorityQueue<usize, OrderedFloat<f64>> =
            DoublePriorityQueue::new();
        let mut total_cost = 0.0;

        for (i, (trigram, weight)) in trigrams.iter().enumerate() {
            // Skip if any key should be ignored
            if self.should_ignore_key(trigram.0)
                || self.should_ignore_key(trigram.1)
                || self.should_ignore_key(trigram.2)
            {
                continue;
            }

            let (is_redirect, is_weak) = classify_redirect(trigram.0, trigram.1, trigram.2);

            if !is_redirect || !self.filter.should_count(is_weak) {
                continue;
            }

            let cost = weight * self.base_cost;
            total_cost += cost;

            worst_queue.push(i, OrderedFloat(cost));

            if worst_queue.len() > n_worst {
                worst_queue.pop_min();
            }
        }

        let worst_msgs: Vec<String> = worst_queue
            .into_sorted_iter()
            .rev()
            .filter(|(_, cost)| cost.into_inner() > 0.0)
            .map(|(i, cost)| {
                let (gram, weight) = trigrams[i];
                let freq_pct = 100.0 * weight / total_weight;
                let cost_pct = 100.0 * cost.into_inner() / total_cost;
                let percentages = format_percentages(cost_pct, freq_pct);
                let trigram_str = format!("{}{}{}", gram.0, gram.1, gram.2);
                format!("{} {}", visualize_whitespace(&trigram_str), percentages)
            })
            .collect();

        let msg = if worst_msgs.is_empty() {
            None
        } else {
            Some(worst_msgs.join(", "))
        };

        (total_cost, msg)
    }
}

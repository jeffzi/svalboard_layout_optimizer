//! Directional full scissoring metric for adjacent finger movements.
//!
//! ## Core Principle
//!
//! Identifies uncomfortable "full scissor" motions where adjacent fingers move in opposing
//! directions. Penalties are based purely on the biomechanical discomfort of the motion
//! pattern itself, independent of key costs:
//!
//! ```
//! penalty = cost × finger_factor × freq_multiplier
//! ```
//!
//! Where:
//! - `cost`: Base cost representing inherent biomechanical discomfort of the motion type
//! - `finger_factor`: Max of the two fingers' factors (weaker finger dominates)
//! - `freq_multiplier`: Optional high-frequency bigram penalty
//!
//! Costs are configured per movement type in the evaluation metrics configuration.
//!
//! ## Movement Classification
//!
//! **Full Scissor Vertical** - Opposite vertical directions (North ↔ South)
//! **Full Scissor Squeeze** - Fingers moving toward each other (In ↔ Out, inward motion - more uncomfortable)
//! **Full Scissor Splay** - Fingers moving apart (In ↔ Out, outward motion - less uncomfortable)
//!
//! ## Configuration
//!
//! Each movement type has its own configuration:
//! - `vertical.cost`: Base cost for vertical scissors (North ↔ South)
//! - `squeeze.cost`: Base cost for squeeze motion (fingers moving inward)
//! - `splay.cost`: Base cost for splay motion (fingers moving outward)
//! - `<type>.finger_factors`: Optional per-finger multipliers (e.g., pinky scissors worse than index)
//! - `critical_bigram_fraction`: Frequency threshold for high-penalty bigrams (optional)
//! - `critical_bigram_factor`: Multiplier for high-frequency bigrams (optional)

use super::{
    is_adjacent_fingers,
    scissor_base::{ScissorCategory, ScissorCompute, ScissorMetric},
    BigramMetric,
};

use ahash::AHashMap;
use colored::Colorize;
use keyboard_layout::{
    key::{Direction::*, Finger},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FsbCategory {
    Vertical,
    Squeeze,
    Splay,
}

impl ScissorCategory for FsbCategory {
    fn display_order() -> &'static [Self] {
        &[
            FsbCategory::Vertical,
            FsbCategory::Squeeze,
            FsbCategory::Splay,
        ]
    }

    fn display_name(&self) -> String {
        match self {
            FsbCategory::Vertical => "Vertical".underline().to_string(),
            FsbCategory::Squeeze => "Squeeze".underline().to_string(),
            FsbCategory::Splay => "Splay".underline().to_string(),
        }
    }
}

#[derive(Clone, Deserialize, Debug)]
pub struct CategoryParams {
    /// Base cost representing inherent biomechanical discomfort
    pub cost: f64,
    /// Optional per-finger multipliers (e.g., pinky: 1.5, index: 0.75)
    /// Defaults to None (all fingers treated equally)
    #[serde(default)]
    pub finger_factors: Option<AHashMap<Finger, f64>>,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Configuration for Vertical scissors (North-South opposition)
    pub vertical: CategoryParams,
    /// Configuration for Squeeze scissors (fingers moving inward)
    pub squeeze: CategoryParams,
    /// Configuration for Splay scissors (fingers moving outward)
    pub splay: CategoryParams,
    /// Minimum relative bigram frequency to apply heavy penalty (as fraction, e.g., 0.0004 = 0.04%)
    pub critical_bigram_fraction: Option<f64>,
    /// Multiplier for bigrams above critical_bigram_fraction (e.g., 100.0 = 100x penalty)
    pub critical_bigram_factor: Option<f64>,
}

#[derive(Clone, Debug)]
struct FsbCompute {
    vertical_cost: f64,
    squeeze_cost: f64,
    splay_cost: f64,
}

impl ScissorCompute<FsbCategory> for FsbCompute {
    fn compute_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        _layout: &Layout,
    ) -> Option<(f64, FsbCategory)> {
        if !is_adjacent_fingers(k1, k2) {
            return None;
        }

        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        match (dir_from, dir_to) {
            // FSB: Full Scissor Vertical - North-South opposition
            (South, North) | (North, South) => Some((self.vertical_cost, FsbCategory::Vertical)),

            // FSB: Full Scissor Lateral - In-Out opposition (squeeze/splay)
            (In, Out) | (Out, In) => {
                let finger_from = k1.key.finger;
                let finger_to = k2.key.finger;
                let inward_motion = finger_from.numeric_index() > finger_to.numeric_index();
                let is_squeeze = inward_motion ^ (dir_from == Out);

                let (cost, category) = if is_squeeze {
                    (self.squeeze_cost, FsbCategory::Squeeze)
                } else {
                    (self.splay_cost, FsbCategory::Splay)
                };

                Some((cost, category))
            }

            // All other combinations: not full scissors
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Fsb {
    inner: ScissorMetric<FsbCategory, FsbCompute>,
}

/// Merge finger_factors from multiple categories
/// Returns None if all categories have None, otherwise returns union of all factors
fn merge_finger_factors(
    category_factors: &[Option<&AHashMap<Finger, f64>>],
) -> Option<AHashMap<Finger, f64>> {
    let has_any = category_factors.iter().any(|f| f.is_some());
    if !has_any {
        return None;
    }

    let mut merged = AHashMap::new();
    for factors in category_factors.iter().filter_map(|f| *f) {
        merged.extend(factors.iter().map(|(k, v)| (*k, *v)));
    }
    Some(merged)
}

impl Fsb {
    pub fn new(params: &Parameters) -> Self {
        let compute = FsbCompute {
            vertical_cost: params.vertical.cost,
            squeeze_cost: params.squeeze.cost,
            splay_cost: params.splay.cost,
        };

        // Merge finger_factors from all categories
        let merged_finger_factors = merge_finger_factors(&[
            params.vertical.finger_factors.as_ref(),
            params.squeeze.finger_factors.as_ref(),
            params.splay.finger_factors.as_ref(),
        ]);

        Self {
            inner: ScissorMetric::new(
                "FSB",
                params.critical_bigram_fraction,
                params.critical_bigram_factor,
                merged_finger_factors,
                compute,
            ),
        }
    }
}

impl BigramMetric for Fsb {
    fn name(&self) -> &str {
        self.inner.name()
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
        self.inner
            .individual_cost(k1, k2, weight, total_weight, layout)
    }

    fn total_cost(
        &self,
        bigrams: &[((&LayerKey, &LayerKey), f64)],
        total_weight: Option<f64>,
        layout: &Layout,
    ) -> (f64, Option<String>) {
        self.inner.total_cost(bigrams, total_weight, layout)
    }
}

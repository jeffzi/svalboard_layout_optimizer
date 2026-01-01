//! Directional half scissoring metric for adjacent finger movements.
//!
//! ## Core Principle
//!
//! Identifies uncomfortable diagonal and lateral motions where adjacent fingers move in
//! partially opposing directions. Penalties are based purely on the biomechanical discomfort
//! of the motion pattern itself, independent of key costs:
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
//! **Diagonal** - Diagonal movements with reduced conflict:
//! - Lateral + Vertical: One finger moves laterally (In/Out), other vertically (North/South)
//!
//! **Lateral** - Lateral displacement:
//! - Lateral + Center: One finger moves laterally (In/Out), other presses Center
//!
//! ## Configuration
//!
//! Each movement type has its own configuration:
//! - `diagonal.cost`: Base cost for diagonal movements (lateral+vertical)
//! - `lateral.cost`: Base cost for lateral movements (lateral+center)
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
pub enum HsbCategory {
    Diagonal,
    Lateral,
}

impl ScissorCategory for HsbCategory {
    fn display_order() -> &'static [Self] {
        &[HsbCategory::Diagonal, HsbCategory::Lateral]
    }

    fn display_name(&self) -> String {
        match self {
            HsbCategory::Diagonal => "Diagonal".underline().to_string(),
            HsbCategory::Lateral => "Lateral".underline().to_string(),
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
    /// Configuration for Diagonal scissors (lateral+vertical)
    pub diagonal: CategoryParams,
    /// Configuration for Lateral scissors (lateral+center)
    pub lateral: CategoryParams,
    /// Minimum relative bigram frequency to apply heavy penalty (as fraction, e.g., 0.0004 = 0.04%)
    pub critical_bigram_fraction: Option<f64>,
    /// Multiplier for bigrams above critical_bigram_fraction (e.g., 100.0 = 100x penalty)
    pub critical_bigram_factor: Option<f64>,
}

#[derive(Clone, Debug)]
struct HsbCompute {
    diagonal_cost: f64,
    lateral_cost: f64,
}

impl ScissorCompute<HsbCategory> for HsbCompute {
    fn compute_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        _layout: &Layout,
    ) -> Option<(f64, HsbCategory)> {
        if !is_adjacent_fingers(k1, k2) {
            return None;
        }

        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        match (dir_from, dir_to) {
            // HSB: Half Scissor - Diagonal movements (lateral + vertical)
            (In, North)
            | (Out, North)
            | (North, In)
            | (North, Out)
            | (In, South)
            | (Out, South)
            | (South, In)
            | (South, Out) => Some((self.diagonal_cost, HsbCategory::Diagonal)),

            // Lateral - Lateral displacement with center
            (In, Center) | (Out, Center) | (Center, In) | (Center, Out) => {
                Some((self.lateral_cost, HsbCategory::Lateral))
            }

            // All other combinations: not considered half scissors or lateral
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hsb {
    inner: ScissorMetric<HsbCategory, HsbCompute>,
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

impl Hsb {
    pub fn new(params: &Parameters) -> Self {
        let compute = HsbCompute {
            diagonal_cost: params.diagonal.cost,
            lateral_cost: params.lateral.cost,
        };

        // Merge finger_factors from all categories
        let merged_finger_factors = merge_finger_factors(&[
            params.diagonal.finger_factors.as_ref(),
            params.lateral.finger_factors.as_ref(),
        ]);

        Self {
            inner: ScissorMetric::new(
                "HSB",
                params.critical_bigram_fraction,
                params.critical_bigram_factor,
                merged_finger_factors,
                compute,
            ),
        }
    }
}

impl BigramMetric for Hsb {
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

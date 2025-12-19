//! Sympathetic movement metric for adjacent finger movements.
//!
//! ## Core Principle
//!
//! Promotes sympathetic finger movements where adjacent fingers move in the same direction.
//! This works WITH the natural mechanical coupling of fingers (especially middle-ring and
//! ring-pinky which share flexor digitorum profundus tendons).
//!
//! Adjacent fingers moving in **identical directions** are biomechanically comfortable:
//! - `(North, North)` - both extending up
//! - `(South, South)` - both flexing down
//! - `(Center, Center)` - both at rest
//! - `(In, In)` - both moving inward
//! - `(Out, Out)` - both moving outward
//!
//! This metric **penalizes non-identical direction** movements between adjacent fingers.
//!
//! ## Research Basis
//!
//! Based on biomechanical research on finger coupling:
//! - **Direction independence**: Flexion (South) > Center > Extension (North) ≈ Lateral (In/Out)
//! - **Finger-pair coupling**: Ring-Pinky (31-64%) > Middle-Ring (37-52%) > Index-Middle (21-28%)
//! - **Finger order matters**: The second finger in a bigram is "enslaved" by the first.
//!   Middle→Ring (ring enslaved, 20-25% independence) is harder than Ring→Middle (~31%).
//!
//! ## Formula
//!
//! ```text
//! cost = weight × max(direction_cost[dir1], direction_cost[dir2]) × finger_pair_factor
//! ```
//!
//! The max formula reflects that the worst direction dominates - having one "easy" direction
//! (flexion) doesn't reduce the conflict when fingers move in different directions.
//!
//! ## Configuration
//!
//! - `direction_costs`: Per-direction cost (lower = more independent).
//!   Suggested: Center (0.0), South (0.0), North (1.0), In (1.2), Out (1.2)
//! - `finger_pair_factors`: Ordered finger-pair coupling multipliers.
//!   Both directions must be specified explicitly (no fallback).

use super::{is_adjacent_fingers, BigramMetric};

use ahash::AHashMap;
use keyboard_layout::{
    key::{Direction, Finger},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Per-direction cost (lower = more independent).
    /// Formula uses max(cost[dir1], cost[dir2]) - worst direction dominates.
    /// Suggested: Center (0.0), South (0.0), North (1.0), In (1.2), Out (1.2)
    pub direction_costs: AHashMap<Direction, f64>,

    /// Ordered finger-pair coupling multipliers.
    /// Key is (first_finger, second_finger) - order matters!
    /// Both directions must be specified (e.g., [Middle, Ring] AND [Ring, Middle]).
    /// Use 0.0 to disable a pair entirely.
    #[serde(default)]
    pub finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

#[derive(Clone, Debug)]
pub struct Sympathetic {
    direction_costs: AHashMap<Direction, f64>,
    finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

impl Sympathetic {
    pub fn new(params: &Parameters) -> Self {
        Self {
            direction_costs: params.direction_costs.clone(),
            finger_pair_factors: params.finger_pair_factors.clone(),
        }
    }

    /// Lookup finger pair factor with ordered key (order matters).
    /// The key is (first_finger, second_finger) in bigram sequence.
    /// Research shows the second finger is "enslaved" by the first,
    /// so Middle→Ring is harder than Ring→Middle.
    #[inline]
    fn finger_pair_factor(&self, f1: Finger, f2: Finger) -> f64 {
        if let Some(ref factors) = self.finger_pair_factors {
            factors.get(&(f1, f2)).copied().unwrap_or(1.0)
        } else {
            1.0
        }
    }
}

impl BigramMetric for Sympathetic {
    fn name(&self) -> &str {
        "Sympathetic"
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
        // Only applies to adjacent fingers on the same hand (no thumbs)
        if !is_adjacent_fingers(k1, k2) {
            return Some(0.0);
        }

        let dir1 = k1.key.direction;
        let dir2 = k2.key.direction;

        // Identical direction = sympathetic = zero cost
        if dir1 == dir2 {
            return Some(0.0);
        }

        // Get finger pair factor (symmetric lookup)
        let pair_factor = self.finger_pair_factor(k1.key.finger, k2.key.finger);

        // If pair is disabled (factor = 0), skip
        if pair_factor == 0.0 {
            return Some(0.0);
        }

        // Direction cost: take the maximum of both costs
        // The worst direction dominates - having one "easy" direction (flexion)
        // doesn't reduce the conflict when fingers move in different directions
        let cost1 = self.direction_costs.get(&dir1).copied().unwrap_or(1.0);
        let cost2 = self.direction_costs.get(&dir2).copied().unwrap_or(1.0);
        let direction_cost = cost1.max(cost2);

        Some(weight * direction_cost * pair_factor)
    }
}

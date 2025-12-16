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
//!
//! ## Configuration
//!
//! - `direction_costs`: Per-direction independence cost (lower = more independent)
//! - `finger_pair_factors`: Per finger-pair coupling multipliers (symmetric lookup)

use super::{is_adjacent_fingers, BigramMetric};

use ahash::AHashMap;
use keyboard_layout::{
    key::{Direction, Finger},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Per-direction independence cost (lower = more independent).
    /// Formula uses cost[dir1] × cost[dir2], so independent movements reduce coupling.
    /// Research suggests: Center (0.0) < South/flexion (0.2) < North/extension = In/Out/lateral (1.0)
    pub direction_costs: AHashMap<Direction, f64>,

    /// Finger-pair coupling multipliers (symmetric - order doesn't matter).
    /// Use 0.0 to disable a pair entirely.
    /// Research suggests: Ring-Pinky (1.5) > Middle-Ring (1.4) > Index-Middle (can be disabled)
    #[serde(default)]
    pub finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

#[derive(Clone, Debug)]
pub struct Sympathetic {
    direction_costs: AHashMap<Direction, f64>,
    finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

/// Normalize finger pair to canonical order for symmetric lookup.
/// Uses numeric index to ensure consistent ordering.
#[inline]
fn normalize_finger_pair(f1: Finger, f2: Finger) -> (Finger, Finger) {
    if (f1 as u8) <= (f2 as u8) {
        (f1, f2)
    } else {
        (f2, f1)
    }
}

impl Sympathetic {
    pub fn new(params: &Parameters) -> Self {
        Self {
            direction_costs: params.direction_costs.clone(),
            finger_pair_factors: params.finger_pair_factors.clone(),
        }
    }

    /// Lookup finger pair factor with symmetric key (order doesn't matter).
    #[inline]
    fn finger_pair_factor(&self, f1: Finger, f2: Finger) -> f64 {
        if let Some(ref factors) = self.finger_pair_factors {
            let key = normalize_finger_pair(f1, f2);
            factors.get(&key).copied().unwrap_or(1.0)
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

        // Direction cost: multiply both costs
        // This naturally handles Center (0.0) as neutral, and reflects that
        // independent movements (low cost) reduce coupling even when paired
        // with high-coupling movements
        let cost1 = self.direction_costs.get(&dir1).copied().unwrap_or(1.0);
        let cost2 = self.direction_costs.get(&dir2).copied().unwrap_or(1.0);
        let direction_cost = cost1 * cost2;

        Some(weight * direction_cost * pair_factor)
    }
}

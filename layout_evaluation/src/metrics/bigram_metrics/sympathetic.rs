//! Sympathetic movement metric for adjacent finger movements.
//!
//! ## Core Principle
//!
//! Promotes sympathetic finger movements where adjacent fingers move in the same direction.
//! This works WITH the natural mechanical coupling of fingers (especially middle-ring and
//! ring-pinky which share flexor digitorum profundus tendons).
//!
//! Adjacent fingers moving in **identical directions** are comfortable (enslaving helps).
//! This metric **penalizes different direction** movements between adjacent fingers.
//! Center (rest position) is ignored since it's not an actual movement.
//!
//! ## Research Basis
//!
//! Based on biomechanical research on finger enslaving (Zatsiorsky et al. 2000):
//! - Adjacent fingers share flexor digitorum profundus tendons
//! - When one finger moves, it involuntarily pulls adjacent fingers the same way
//! - **Finger-pair coupling**: Ring-Pinky > Middle-Ring > Index-Middle
//! - **Finger order matters**: The second finger is "enslaved" by the first
//! - **Index-Middle excluded**: Index has high independence (~61%), minimal coupling
//!
//! ## Formula
//!
//! ```text
//! cost = weight × finger_pair_factor  (if directions differ)
//! ```
//!
//! ## Configuration
//!
//! - `finger_pair_factors`: Ordered finger-pair coupling multipliers.
//!   Both directions must be specified explicitly (no fallback).

use super::{has_coupling_conflict, is_adjacent_fingers, BigramMetric};

use ahash::AHashMap;
use keyboard_layout::{
    key::Finger,
    layout::{LayerKey, Layout},
};

use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Ordered finger-pair coupling multipliers.
    /// Key is (first_finger, second_finger) - order matters!
    /// Both directions must be specified (e.g., [Middle, Ring] AND [Ring, Middle]).
    /// Index-Middle pairs cannot be configured (always disabled due to high independence).
    #[serde(default)]
    pub finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

/// Check if a finger pair is Index-Middle (in either order).
/// These pairs are always excluded due to high index finger independence.
#[inline]
fn is_index_middle_pair(f1: Finger, f2: Finger) -> bool {
    (f1 == Finger::Index && f2 == Finger::Middle) || (f1 == Finger::Middle && f2 == Finger::Index)
}

#[derive(Clone, Debug)]
pub struct Sympathetic {
    finger_pair_factors: Option<AHashMap<(Finger, Finger), f64>>,
}

impl Sympathetic {
    pub fn new(params: &Parameters) -> Self {
        // Validate that no Index-Middle pairs are configured
        if let Some(ref factors) = params.finger_pair_factors {
            for (f1, f2) in factors.keys() {
                if is_index_middle_pair(*f1, *f2) {
                    panic!(
                        "Index-Middle finger pairs cannot be configured in Sympathetic metric \
                        (always disabled due to high index finger independence). \
                        Remove [Index, Middle] and [Middle, Index] from finger_pair_factors."
                    );
                }
            }
        }

        Self {
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

        // Index-Middle pairs are always excluded (high index independence)
        if is_index_middle_pair(k1.key.finger, k2.key.finger) {
            return Some(0.0);
        }

        let dir1 = k1.key.direction;
        let dir2 = k2.key.direction;

        // Only penalize coupling conflicts (different directions, both "hard")
        if !has_coupling_conflict(dir1, dir2) {
            return Some(0.0);
        }

        // Get finger pair factor (ordered lookup - second finger "enslaved" by first)
        let pair_factor = self.finger_pair_factor(k1.key.finger, k2.key.finger);

        Some(weight * pair_factor)
    }
}

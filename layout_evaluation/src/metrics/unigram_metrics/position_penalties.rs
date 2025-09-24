//! The unigram metric [`PositionPenalties`] penalizes specific letters placed on
//! specific matrix positions with a configurable cost. This is useful for preventing
//! certain letters from being placed on difficult-to-reach keys.

use super::UnigramMetric;

use keyboard_layout::layout::{LayerKey, Layout};

use ahash::AHashMap;
use serde::Deserialize;

/// A tuple representing matrix position: (Column, Row)
type MatrixPosition = (u8, u8);

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Mapping of letters to matrix positions and their penalty costs
    pub penalty_positions: AHashMap<char, AHashMap<MatrixPosition, f64>>,
}

#[derive(Clone, Debug)]
pub struct PositionPenalties {
    penalty_positions: AHashMap<char, AHashMap<MatrixPosition, f64>>,
}

impl PositionPenalties {
    pub fn new(params: &Parameters) -> Self {
        Self {
            penalty_positions: params.penalty_positions.clone(),
        }
    }
}

impl UnigramMetric for PositionPenalties {
    fn name(&self) -> &str {
        "Position Penalties"
    }

    #[inline(always)]
    fn individual_cost(
        &self,
        key: &LayerKey,
        weight: f64,
        _total_weight: f64,
        _layout: &Layout,
    ) -> Option<f64> {
        let symbol = key.symbol;

        if let Some(penalty_map) = self.penalty_positions.get(&symbol) {
            let matrix_pos = (key.key.matrix_position.0, key.key.matrix_position.1);

            if let Some(penalty_cost) = penalty_map.get(&matrix_pos) {
                log::trace!(
                    "Penalty: Symbol '{}' at position {:?}, Weight: {:>12.2}, Penalty: {:>8.4}, Cost: {:>14.4}",
                    symbol, matrix_pos, weight, penalty_cost, weight * penalty_cost
                );
                return Some(weight * penalty_cost);
            }
        }

        Some(0.0)
    }
}

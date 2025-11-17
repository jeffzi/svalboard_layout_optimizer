//! Redirect metric that penalizes redirects (excluding weak redirects).
//! A redirect is a one-handed trigram with a direction change (e.g., inward->outward or outward->inward)
//! that involves the index finger or thumb.

use super::{
    redirect_base::{NormalRedirectFilter, RedirectMetric},
    TrigramMetric,
};
use keyboard_layout::layout::{LayerKey, Layout};
use serde::Deserialize;

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    /// Base cost multiplier for each redirect. Default: 1.0
    pub base_cost: Option<f64>,
    /// Ignore redirects involving thumb keys. Default: true
    pub ignore_thumbs: Option<bool>,
    /// Ignore redirects involving modifier keys. Default: true
    pub ignore_modifiers: Option<bool>,
}

#[derive(Clone, Debug)]
pub struct Redirects {
    inner: RedirectMetric<NormalRedirectFilter>,
}

impl Redirects {
    pub fn new(params: &Parameters) -> Self {
        Self {
            inner: RedirectMetric::new(
                "Redirects",
                NormalRedirectFilter,
                params.base_cost.unwrap_or(1.0),
                params.ignore_thumbs.unwrap_or(true),
                params.ignore_modifiers.unwrap_or(true),
            ),
        }
    }
}

impl TrigramMetric for Redirects {
    fn name(&self) -> &str {
        self.inner.name()
    }

    #[inline(always)]
    fn individual_cost(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
        k3: &LayerKey,
        weight: f64,
        total_weight: f64,
        layout: &Layout,
    ) -> Option<f64> {
        self.inner
            .individual_cost(k1, k2, k3, weight, total_weight, layout)
    }

    fn total_cost(
        &self,
        trigrams: &[((&LayerKey, &LayerKey, &LayerKey), f64)],
        total_weight: Option<f64>,
        layout: &Layout,
    ) -> (f64, Option<String>) {
        self.inner.total_cost(trigrams, total_weight, layout)
    }
}

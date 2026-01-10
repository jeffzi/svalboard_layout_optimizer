use super::TrigramMetric;

use colored::Colorize;
use keyboard_layout::{
    key::{Direction, Finger, Hand},
    layout::{LayerKey, Layout},
};

use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TrigramCategory {
    BigramRollIn,
    BigramRollOut,
    RollIn,
    RollOut,
    Alternation,
    Redirect,
    WeakRedirect,
    Other,
}

#[derive(Clone, Deserialize, Debug)]
pub struct Parameters {
    pub ignore_modifiers: bool,
    pub ignore_thumbs: bool,
    /// List of same-finger movements to track separately within bigram rolls (e.g., [[Center, South], [In, South]])
    #[serde(default = "default_same_finger_rolls")]
    pub same_finger_rolls: Vec<(Direction, Direction)>,
}

fn default_same_finger_rolls() -> Vec<(Direction, Direction)> {
    vec![]
}

#[derive(Clone, Debug)]
pub struct TrigramStats {
    ignore_modifiers: bool,
    ignore_thumbs: bool,
    same_finger_rolls: Vec<(Direction, Direction)>,
}

impl TrigramStats {
    pub fn new(params: &Parameters) -> Self {
        Self {
            ignore_modifiers: params.ignore_modifiers,
            ignore_thumbs: params.ignore_thumbs,
            same_finger_rolls: params.same_finger_rolls.clone(),
        }
    }

    fn should_ignore_key(&self, key: &LayerKey) -> bool {
        (self.ignore_thumbs && key.key.finger == Finger::Thumb)
            || (self.ignore_modifiers && key.is_modifier.is_some())
    }

    /// Check if this same-finger movement matches any configured same-finger rolls
    /// Returns Some((Direction, Direction)) if it matches, None otherwise
    fn check_same_finger_roll(
        &self,
        k1: &LayerKey,
        k2: &LayerKey,
    ) -> Option<(Direction, Direction)> {
        let dir_from = k1.key.direction;
        let dir_to = k2.key.direction;

        for &(from, to) in &self.same_finger_rolls {
            if dir_from == from && dir_to == to {
                return Some((from, to));
            }
        }
        None
    }

    fn classify_trigram(&self, k1: &LayerKey, k2: &LayerKey, k3: &LayerKey) -> TrigramCategory {
        let h1 = k1.key.hand;
        let h2 = k2.key.hand;
        let h3 = k3.key.hand;

        if h1 == h2 && h2 == h3 {
            // Same hand (all 3 keys) - check roll in/out or redirect
            let (is_roll_in, is_roll_out) = classify_same_hand_roll(k1, k2, k3);

            if is_roll_in {
                return TrigramCategory::RollIn;
            } else if is_roll_out {
                return TrigramCategory::RollOut;
            } else {
                // Not a roll, check for redirect
                let (is_redirect, is_weak) = classify_redirect(k1, k2, k3);
                if is_redirect {
                    return if is_weak {
                        TrigramCategory::WeakRedirect
                    } else {
                        TrigramCategory::Redirect
                    };
                }
            }
        } else if h1 == h3 && h1 != h2 {
            // Alternation (LRL or RLR)
            return TrigramCategory::Alternation;
        } else {
            // Bigram pattern (2,1 or 1,2) - check bigram rolls
            let (is_inward, is_outward) = self.classify_roll(k1, k2, k3);

            if is_inward {
                return TrigramCategory::BigramRollIn;
            } else if is_outward {
                return TrigramCategory::BigramRollOut;
            }
        }

        TrigramCategory::Other
    }

    /// Classify a trigram roll into its category
    /// Returns: (is_inward, is_outward)
    fn classify_roll(&self, k1: &LayerKey, k2: &LayerKey, k3: &LayerKey) -> (bool, bool) {
        let h1 = k1.key.hand;
        let h2 = k2.key.hand;
        let h3 = k3.key.hand;

        let first_roll = h1 == h2 && h2 != h3;
        let second_roll = h1 != h2 && h2 == h3;

        if !(first_roll || second_roll) {
            return (false, false);
        }

        let (kr1, kr2) = if first_roll { (k1, k2) } else { (k2, k3) };

        // Same-finger movements are not considered rolls (handled separately as same_finger_rolls)
        if kr1.key.finger == kr2.key.finger {
            return (false, false);
        }

        // Different fingers: check inward vs outward
        let inwards = match kr1.key.hand {
            Hand::Left => kr1.key.matrix_position.0 < kr2.key.matrix_position.0,
            Hand::Right => kr1.key.matrix_position.0 > kr2.key.matrix_position.0,
        };

        if inwards {
            (true, false)
        } else {
            (false, true)
        }
    }

    /// Extract the bigram pair from a trigram (either first two or last two keys)
    /// Returns Some((k1, k2)) for the bigram part, or None if not a bigram pattern
    fn extract_bigram_pair<'a>(
        &self,
        k1: &'a LayerKey,
        k2: &'a LayerKey,
        k3: &'a LayerKey,
    ) -> Option<(&'a LayerKey, &'a LayerKey)> {
        let h1 = k1.key.hand;
        let h2 = k2.key.hand;
        let h3 = k3.key.hand;

        let first_roll = h1 == h2 && h2 != h3;
        let second_roll = h1 != h2 && h2 == h3;

        if first_roll {
            Some((k1, k2))
        } else if second_roll {
            Some((k2, k3))
        } else {
            None
        }
    }
}

#[inline(always)]
fn inwards(k1: &LayerKey, k2: &LayerKey) -> bool {
    if k1.key.hand == Hand::Left {
        k1.key.matrix_position.0 < k2.key.matrix_position.0
    } else {
        k1.key.matrix_position.0 > k2.key.matrix_position.0
    }
}

/// Check if a trigram is a same-hand roll (all 3 keys on same hand, different fingers, directional)
/// Returns: (is_roll_in, is_roll_out)
fn classify_same_hand_roll(k1: &LayerKey, k2: &LayerKey, k3: &LayerKey) -> (bool, bool) {
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

    // Check if all three movements are in the same direction
    let inwards1 = inwards(k1, k2);
    let inwards2 = inwards(k2, k3);

    let outwards1 = inwards(k2, k1);
    let outwards2 = inwards(k3, k2);

    // Roll in: both movements inward
    if inwards1 && inwards2 {
        return (true, false);
    }

    // Roll out: both movements outward
    if outwards1 && outwards2 {
        return (false, true);
    }

    (false, false)
}

/// Check if a trigram is a redirect: one-handed with direction change
/// Returns: (is_redirect, is_weak_redirect)
fn classify_redirect(k1: &LayerKey, k2: &LayerKey, k3: &LayerKey) -> (bool, bool) {
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

impl TrigramMetric for TrigramStats {
    fn name(&self) -> &str {
        "Trigram Statistics"
    }

    fn total_cost(
        &self,
        trigrams: &[((&LayerKey, &LayerKey, &LayerKey), f64)],
        total_weight: Option<f64>,
        _layout: &Layout,
    ) -> (f64, Option<String>) {
        let mut category_weights: HashMap<TrigramCategory, f64> = HashMap::new();
        let mut same_finger_roll_weights: HashMap<(Direction, Direction), f64> = HashMap::new();
        let mut center_south_roll_weight = 0.0;
        let mut south_south_roll_weight = 0.0;
        let mut center_center_roll_weight = 0.0;
        let mut weak_redirects_weight = 0.0;
        let mut sfs_weight = 0.0;
        let mut valid_trigrams_weight = 0.0;

        let total_trigrams_weight =
            total_weight.unwrap_or_else(|| trigrams.iter().map(|(_, w)| w).sum());

        for ((k1, k2, k3), weight) in trigrams {
            // Check for SFS (Same Finger Skipgram) - k1 and k3 same finger
            if !self.should_ignore_key(k1)
                && !self.should_ignore_key(k3)
                && k1 != k3 // Skip same-key repeats
                && k1.key.hand == k3.key.hand
                && k1.key.finger == k3.key.finger
            {
                sfs_weight += weight;
            }

            // Skip ignored keys for other metrics
            if self.should_ignore_key(k1)
                || self.should_ignore_key(k2)
                || self.should_ignore_key(k3)
            {
                continue;
            }

            valid_trigrams_weight += weight;

            // Check if this trigram contains a same-finger bigram that matches same_finger_rolls
            if let Some((kb1, kb2)) = self.extract_bigram_pair(k1, k2, k3) {
                if kb1.key.hand == kb2.key.hand && kb1.key.finger == kb2.key.finger {
                    if let Some(movement) = self.check_same_finger_roll(kb1, kb2) {
                        *same_finger_roll_weights.entry(movement).or_insert(0.0) += weight;
                    }
                }

                // Track specific direction combinations for adjacent-finger rolls
                if kb1.key.hand == kb2.key.hand && kb1.key.finger != kb2.key.finger {
                    let dir1 = kb1.key.direction;
                    let dir2 = kb2.key.direction;

                    // Center<->South (either direction)
                    if (dir1 == Direction::Center && dir2 == Direction::South)
                        || (dir1 == Direction::South && dir2 == Direction::Center)
                    {
                        center_south_roll_weight += weight;
                    }
                    // South<->South
                    else if dir1 == Direction::South && dir2 == Direction::South {
                        south_south_roll_weight += weight;
                    }
                    // Center<->Center
                    else if dir1 == Direction::Center && dir2 == Direction::Center {
                        center_center_roll_weight += weight;
                    }
                }
            }

            let category = self.classify_trigram(k1, k2, k3);
            *category_weights.entry(category).or_insert(0.0) += weight;

            // Track weak redirects separately for the message
            if category == TrigramCategory::WeakRedirect {
                weak_redirects_weight += weight;
            }
        }

        // Helper to get weight for a category
        let get_weight = |cat: TrigramCategory| *category_weights.get(&cat).unwrap_or(&0.0);

        // Calculate percentages
        let to_pct = |weight| crate::metrics::to_percentage(weight, valid_trigrams_weight);

        let bigram_inward_percentage = to_pct(get_weight(TrigramCategory::BigramRollIn));
        let bigram_outward_percentage = to_pct(get_weight(TrigramCategory::BigramRollOut));
        let roll_in_percentage = to_pct(get_weight(TrigramCategory::RollIn));
        let roll_out_percentage = to_pct(get_weight(TrigramCategory::RollOut));
        let alternation_percentage = to_pct(get_weight(TrigramCategory::Alternation));
        let redirect_percentage =
            to_pct(get_weight(TrigramCategory::Redirect) + weak_redirects_weight);
        let weak_redirect_percentage = to_pct(weak_redirects_weight);
        let other_percentage = to_pct(get_weight(TrigramCategory::Other));
        let sfs_percentage = crate::metrics::to_percentage(sfs_weight, total_trigrams_weight);

        // Calculate total bigram roll weight (including same-finger rolls)
        let same_finger_rolls_total: f64 = same_finger_roll_weights.values().sum();
        let total_bigram_rolls_weight = get_weight(TrigramCategory::BigramRollIn)
            + get_weight(TrigramCategory::BigramRollOut)
            + same_finger_rolls_total;
        let total_bigram_rolls_percentage = to_pct(total_bigram_rolls_weight);

        // Build message with category groups separated by semicolons
        let mut groups = Vec::new();

        // 2-Roll group
        let mut roll_2_parts = Vec::new();
        roll_2_parts.push(format!(
            "{}: {:.1}%",
            "2-Roll Total".underline(),
            total_bigram_rolls_percentage
        ));

        if bigram_inward_percentage > 0.0 {
            roll_2_parts.push(format!(
                "{}: {:.1}%",
                "2-Roll In".underline(),
                bigram_inward_percentage
            ));
        }

        if bigram_outward_percentage > 0.0 {
            roll_2_parts.push(format!(
                "{}: {:.1}%",
                "2-Roll Out".underline(),
                bigram_outward_percentage
            ));
        }

        // Add same-finger roll movements to 2-Roll group
        for ((dir_from, dir_to), weight) in same_finger_roll_weights.iter() {
            let percentage = to_pct(*weight);
            if percentage > 0.0 {
                let movement_label = format!("2-Roll {:?}→{:?}", dir_from, dir_to);
                roll_2_parts.push(format!(
                    "{}: {:.1}%",
                    movement_label.underline(),
                    percentage
                ));
            }
        }

        // Add direction-specific rolls (Center<->South, South<->South, Center<->Center)
        let center_south_percentage = to_pct(center_south_roll_weight);
        if center_south_percentage > 0.0 {
            roll_2_parts.push(format!(
                "{}: {:.1}%",
                "Center<->South".underline(),
                center_south_percentage
            ));
        }
        let south_south_percentage = to_pct(south_south_roll_weight);
        if south_south_percentage > 0.0 {
            roll_2_parts.push(format!(
                "{}: {:.1}%",
                "South<->South".underline(),
                south_south_percentage
            ));
        }
        let center_center_percentage = to_pct(center_center_roll_weight);
        if center_center_percentage > 0.0 {
            roll_2_parts.push(format!(
                "{}: {:.1}%",
                "Center<->Center".underline(),
                center_center_percentage
            ));
        }

        groups.push(roll_2_parts.join(", "));

        // 3-Roll group
        let mut roll_3_parts = Vec::new();
        if roll_in_percentage > 0.0 {
            roll_3_parts.push(format!(
                "{}: {:.1}%",
                "3-Roll In".underline(),
                roll_in_percentage
            ));
        }

        if roll_out_percentage > 0.0 {
            roll_3_parts.push(format!(
                "{}: {:.1}%",
                "3-Roll Out".underline(),
                roll_out_percentage
            ));
        }
        if !roll_3_parts.is_empty() {
            groups.push(roll_3_parts.join(", "));
        }

        // Alt group
        if alternation_percentage > 0.0 {
            groups.push(format!(
                "{}: {:.1}%",
                "Alt".underline(),
                alternation_percentage
            ));
        }

        // Redirect group
        let mut redirect_parts = Vec::new();
        if redirect_percentage > 0.0 {
            redirect_parts.push(format!(
                "{}: {:.1}%",
                "Redirect".underline(),
                redirect_percentage
            ));
        }

        if weak_redirect_percentage > 0.0 {
            redirect_parts.push(format!(
                "{}: {:.1}%",
                "Weak redirect".underline(),
                weak_redirect_percentage
            ));
        }
        if !redirect_parts.is_empty() {
            groups.push(redirect_parts.join(", "));
        }

        // Other group
        if other_percentage > 0.0 {
            groups.push(format!("{}: {:.1}%", "Other".underline(), other_percentage));
        }

        // SFS group
        if sfs_percentage > 0.0 {
            groups.push(format!("{}: {:.1}%", "SFS".underline(), sfs_percentage));
        }

        let message = groups.join("; ");

        // Return 0 cost since this is informational only
        (0.0, Some(message))
    }
}

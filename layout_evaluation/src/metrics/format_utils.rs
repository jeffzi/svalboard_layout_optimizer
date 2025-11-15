//! Utility functions for formatting metric output

use colored::Colorize;

/// Format cost and frequency percentages with dimmed color
///
/// Returns a formatted string like "27.5%|0.05%" with dimmed gray color
pub fn format_percentages(cost_percent: f64, freq_percent: f64) -> String {
    format!("{:.1}%|{:.2}%", cost_percent, freq_percent)
        .truecolor(150, 150, 150)
        .to_string()
}

/// Replace whitespace characters with visible symbols for display
///
/// Replaces space with "␣" to make whitespace visible in output
pub fn visualize_whitespace(s: &str) -> String {
    s.replace(' ', "␣")
}

/// Determine if an n-gram should be shown in worst messages based on its percentages
///
/// Returns true if the n-gram should be displayed (both percentages round to > 0.00).
/// Filters out n-grams where either cost or frequency percentage rounds to 0.00% (to 2 decimals).
///
/// # Arguments
/// * `cost_pct` - Cost percentage value
/// * `freq_pct` - Optional frequency percentage value (None for metrics that don't track frequency)
///
/// # Returns
/// true if cost_pct >= 0.005 AND (freq_pct.is_none() OR freq_pct >= 0.005) (i.e., rounds to >= 0.01%)
pub fn should_show_ngram(cost_pct: f64, freq_pct: Option<f64>) -> bool {
    const THRESHOLD: f64 = 0.005; // Rounds to 0.01% when displayed as .2 decimal places

    cost_pct >= THRESHOLD && freq_pct.map_or(true, |f| f >= THRESHOLD)
}

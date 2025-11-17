#!/usr/bin/env python3

import csv
import json
import re
from pathlib import Path
from typing import Callable, TextIO
from urllib.parse import quote
import unicodedata

import typer
from rich.console import Console

# =============================================================================
# CONSTANTS
# =============================================================================

# Frequency and formatting
DEFAULT_FREQ_THRESHOLD = 0.01  # 1% - minimum frequency to include in reports
BALANCE_METRIC_DECIMALS = 1  # decimal places for balance metrics
DEFAULT_METRIC_DECIMALS = 2  # decimal places for most metrics

# Metrics that should have low-frequency entries filtered
METRICS_TO_FILTER = ["SFB", "Manual Bigram Penalty", "Scissors", "FSB", "HSB"]

BIGRAM_STAT_PATTERNS = ["SFB", "Vertical", "Squeeze", "Splay", "Diagonal", "Lateral"]

TRIGRAM_STAT_PATTERNS = [
    "2-Roll Total",
    "2-Roll In",
    "2-Roll Out",
    "2-Roll Center→South",
    "Alt",
    "Redirect",
    "Weak redirect",
    "SFS",
]

SCISSOR_PATTERNS = ["Vertical", "Squeeze", "Splay", "Diagonal", "Lateral"]

ROLL_PATTERNS = ["2-Roll Total", "2-Roll In", "2-Roll Out", "2-Roll Center→South"]

SECTION_RE = re.compile(r"^\s*([^:;]+):\s*(.*)$")
ENTRY_RE = re.compile(
    r"(?:(?<=^)|(?<=,)|(?<=;)|(?<=:))\s*"  # entry boundary
    r"(?P<token>.+?)\s*"  # token (lazy)
    r"(?P<cost>\d+(?:\.\d+)?)%\|"  # cost%
    r"(?P<freq>\d+(?:\.\d+)?)%",  # freq%
    re.S,
)

METRICS_ORDER = [
    ("Total Cost", "total_cost", "number", 1),
    ("Hands Disbalance", "Hand Disbalance", "message_only", None),
    ("Finger Disbalance", "Finger Balance", "message_only", None),
    ("SFB", "SFB", "number", 2),
    ("Scissors", "Scissors", "number", 2),
    ("FSB", "FSB", "number", 2),
    ("HSB", "HSB", "number", 2),
    ("SFS", "SFS", "number", 2),
    ("Redirects", "Redirects", "number", 2),
    ("Weak Redirect", "Weak Redirect", "number", 2),
    ("Key Costs", "Key Costs", "number", 2),
    ("Manual Bigram Penalty", "Manual Bigram Penalty", "number", 2),
    ("SFB Worst", "SFB", "worst_only", None),
    ("Scissors Worst", "Scissors", "worst_only", None),
    ("FSB Worst", "FSB", "worst_only", None),
    ("HSB Worst", "HSB", "worst_only", None),
    ("SFS Worst", "SFS", "worst_only", None),
    ("Redirects Worst", "Redirects", "worst_only", None),
    ("Weak Redirect Worst", "Weak Redirect", "worst_only", None),
    ("Movement Pattern Worst", "Movement Pattern", "worst_only", None),
    ("Manual Bigram Penalty Worst", "Manual Bigram Penalty", "worst_only", None),
    ("Secondary Bigrams Worst", "Secondary Bigrams", "worst_only", None),
    ("Trigrams Worst", "No Handswitch in Trigram", "worst_only", None),
    ("Movement Pattern", "Movement Pattern", "number", 2),
    ("Bigram Statistics", "Bigram Statistics", "message_only", None),
    ("Trigram Statistics", "Trigram Statistics", "message_only", None),
]

COLUMN_HEADERS = ["Layout", "Homerow"] + [display for display, *_ in METRICS_ORDER]

METRICS_DESCRIPTIONS = {
    "Hands Disbalance": "Left and right hand balance",
    "Finger Balance": "Left pinky -> left index and then right index -> right pinky",
    "Finger Disbalance": "Left pinky -> left index and then right index -> right pinky",
    "Homerow": "The center keys of each finger cluster",
    "SFB": "Same Finger Bigram metric that evaluates the comfort of same finger bigrams. Center to south bigrams are good here.",
    "SFB %": "Same Finger Bigrams percentage (excluding good Center→South movements)",
    "Scissors %": "Scissor movements between adjacent fingers:\n  - **Vertical**: North-South opposition\n  - **Squeeze**: Fingers moving inward (more uncomfortable)\n  - **Splay**: Fingers moving outward (less uncomfortable)\n  - **Diagonal**: Lateral + vertical movements\n  - **Lateral**: Lateral displacement with center",
    "FSB": "Full Scissor Bigram metric for the most uncomfortable scissor movements: Vertical (North↔South opposition), Squeeze (fingers moving inward), and Splay (fingers moving outward). Penalties based on inherent biomechanical discomfort.",
    "HSB": "Half Scissor Bigram metric for less severe scissor movements: Diagonal (lateral+vertical movements) and Lateral (lateral+center movements). Penalties based on inherent biomechanical discomfort.",
    "SFS": "Same Finger Skipgram (SFS) metric that evaluates the comfort of skipgrams typed with the same finger. Skipgrams are generally uncomfortable because the middle keystroke interrupts the finger movement pattern.",
    "SFS %": "Same Finger Skipgram percentage",
    "2-Rolls": "Roll statistics for 3-key sequences with hand alternation where the 2 same-hand keys use different fingers:\n  - **Total**: All rolls combined\n  - **In**: Inward rolls (towards the index finger)\n  - **Out**: Outward rolls (towards the pinky)\n  - **Center→South**: Same-finger vertical movement from center row to south row in a roll pattern",
    "Alternation %": "Hand alternation percentage",
    "Redirect": "Penalizes one-handed trigrams with direction changes (e.g., inward→outward or outward→inward) that involve the index finger or thumb.",
    "Redirect %": "Redirect percentage (direction changes involving index finger or thumb)",
    "Weak Redirect": "Penalizes one-handed trigrams with direction changes that do NOT involve the index finger or thumb, making them harder to execute.",
    "Weak Redirect %": "Weak redirect percentage (direction changes NOT involving index finger or thumb)",
    "Key Costs": "Penalizes using keys that are harder to reach based on position (direction and finger)",
    "Manual Bigram Penalty": "Applies specific penalties to manually defined key combinations that are hard to describe otherwise, such as same-key repeats on pinky fingers",
    "Bigram Statistics": "Informational statistics showing percentages of various bigram categories",
    "Trigram Statistics": "Informational statistics showing rolls, alternation, redirects, and skipgrams",
}


def generate_metrics_description(filtered_headers: list[str]) -> str:
    """Generate metrics description section based on metrics actually present in the output."""
    # Collect unique metrics that have descriptions
    metrics_to_describe = set()
    for header in filtered_headers:
        # Map headers to their base metric names
        if header in METRICS_DESCRIPTIONS:
            metrics_to_describe.add(header)
        elif "Worst" in header:
            base_name = header.replace(" Worst", "")
            if base_name in METRICS_DESCRIPTIONS:
                metrics_to_describe.add(base_name)

    if not metrics_to_describe:
        return ""

    lines = ["## Metrics Description\n"]
    for metric in sorted(metrics_to_describe):
        if metric in METRICS_DESCRIPTIONS:
            lines.append(f"**{metric}**: {METRICS_DESCRIPTIONS[metric]}\n")

    return "\n".join(lines)


# =============================================================================
# CORPUS HANDLING
# =============================================================================


def get_corpus_paths(corpus_name: str) -> tuple[Path, Path, Path]:
    """Get corpus directory and ngrams file paths."""
    script_dir = Path(__file__).parent
    project_root = script_dir.parent.parent
    corpus_dir = project_root / "ngrams" / corpus_name
    ngrams_file = corpus_dir / "2-grams.txt"
    return project_root, corpus_dir, ngrams_file


def load_bigram_frequencies(corpus_name: str) -> dict[str, float]:
    """Load bigram frequencies from corpus 2-grams.txt file."""
    _, _, ngrams_file = get_corpus_paths(corpus_name)

    frequencies = {}
    with open(ngrams_file, encoding="utf-8") as f:
        for line in f:
            parts = line.strip().split(" ")
            if len(parts) >= 2:
                freq = float(parts[0])
                bigram = parts[1]
                if len(bigram) == 2:
                    frequencies[bigram] = freq
    return frequencies


def validate_corpus(corpus_name: str) -> str:
    """Validate that corpus exists and return the corpus name."""
    if not corpus_name:
        return corpus_name

    project_root, corpus_dir, _ = get_corpus_paths(corpus_name)

    if not corpus_dir.exists():
        ngrams_dir = project_root / "ngrams"
        available_corpora = (
            sorted([d.name for d in ngrams_dir.iterdir() if d.is_dir()])
            if ngrams_dir.exists()
            else []
        )
        available = (
            f" Available: {', '.join(available_corpora)}" if available_corpora else ""
        )
        raise typer.BadParameter(f"Corpus '{corpus_name}' not found.{available}")

    return corpus_name


# =============================================================================
# MESSAGE PROCESSING
# =============================================================================


def drop_low_freq_entries(
    message: str | None, threshold: float = DEFAULT_FREQ_THRESHOLD
) -> str:
    """
    Remove entries whose frequency (right side of '|') is < threshold (%).
    Preserves tokens that include punctuation such as commas (e.g., 'l,', 'o,', 'a.').
    Drops empty sections.
    """
    if message is None:
        return ""
    out_sections = []
    for raw_sec in (s.strip() for s in message.split(";")):
        if not raw_sec:
            continue
        m = SECTION_RE.match(raw_sec)
        if m:
            label, body = m.group(1).strip(), m.group(2)
        else:
            label, body = None, raw_sec

        kept = []
        for em in ENTRY_RE.finditer(body):
            token = em.group("token").strip()  # keep punctuation like ',' or '.'
            cost = em.group("cost")
            freq = float(em.group("freq"))
            if freq >= threshold:
                kept.append(f"{token} {cost}%|{em.group('freq')}%")

        if kept:
            out_sections.append(
                f"{label}: {', '.join(kept)}" if label else ", ".join(kept)
            )

    return "; ".join(out_sections)


def clean_message(message: str | None, metric_name: str = "") -> str:
    """Clean message for data storage."""
    if message is None:
        return ""
    prefixes = ["Finger loads % (no thumb): ", "Hand loads % (no thumb): ", "Worst: "]
    for prefix in prefixes:
        message = message.removeprefix(prefix)

    # Format percentages and other numbers
    match metric_name:
        case "Hand Disbalance" | "Finger Balance":
            decimals = BALANCE_METRIC_DECIMALS
            message = re.sub(
                r"(\d+\.\d+)(?!%\))",
                lambda m: f"{float(m.group(1)):.{decimals}f}",
                message,
            )
        case "Trigram Statistics":
            # Remove unwanted sub-metrics
            decimals = DEFAULT_METRIC_DECIMALS
            message = re.sub(
                r"(\d+\.\d+)%,",
                lambda m: f"{float(m.group(1)):.{decimals}f}%,",
                message,
            )
            message = re.sub(r",\s*3-Roll In:\s*[\d.]+%", "", message)
            message = re.sub(r",\s*3-Roll Out:\s*[\d.]+%", "", message)
            message = re.sub(r";\s*Other:\s*[\d.]+%", "", message)
        case _:
            decimals = DEFAULT_METRIC_DECIMALS
            message = re.sub(
                r"(\d+\.\d+)%,",
                lambda m: f"{float(m.group(1)):.{decimals}f}%,",
                message,
            )

    return message.strip()


def format_message_for_markdown(message: str | None, metric_name: str = "") -> str:
    """Add markdown formatting tags to message for display."""
    if message is None:
        return ""
    match metric_name:
        case "Bigram Statistics":
            for pattern in BIGRAM_STAT_PATTERNS:
                message = message.replace(f"{pattern}:", f"<u>{pattern}</u>:")
        case "Trigram Statistics":
            for pattern in TRIGRAM_STAT_PATTERNS:
                message = message.replace(f"{pattern}:", f"<u>{pattern}</u>:")
        case "Scissors":
            # Format extracted scissor statistics
            for pattern in SCISSOR_PATTERNS:
                message = message.replace(f"{pattern}:", f"<u>{pattern}</u>:")
        case "2-Rolls":
            # Format extracted roll statistics
            for pattern in ["Total", "In", "Out", "Center→South"]:
                message = message.replace(f"{pattern}:", f"<u>{pattern}</u>:")

    return message


# =============================================================================
# LAYOUT PARSING AND PROCESSING
# =============================================================================


def process_layout_metrics(
    result: dict, bigram_frequencies: dict[str, float]
) -> dict[str, dict]:
    """Process all metrics for a single layout result."""
    metrics_data = {}

    for individual_result in result["details"]["individual_results"]:
        for metric_cost in individual_result["metric_costs"]:
            core = metric_cost["core"]
            message = core["message"]

            if core["name"] in METRICS_TO_FILTER and bigram_frequencies:
                message = drop_low_freq_entries(message)

            metrics_data[core["name"]] = {
                "cost": metric_cost["weighted_cost"],
                "message": message,
            }

    return metrics_data


def extract_statistic_value(message: str, pattern: str) -> str:
    """Extract a specific statistic value from a message string."""
    # Match pattern like "SFB: 0.26%" or "<u>Alt</u>: 41.2%"
    # Handle both with and without underline tags
    escaped_pattern = re.escape(pattern)
    match = re.search(rf"(?:<u>)?{escaped_pattern}(?:</u>)?:\s*([\d.]+%)", message)
    return match.group(1) if match else ""


def extract_bigram_sfb(message: str) -> str:
    """Extract SFB percentage from Bigram Statistics."""
    return extract_statistic_value(message, "SFB")


def extract_bigram_scissors(message: str) -> str:
    """Extract all scissor statistics (Vertical, Squeeze, Splay, Diagonal, Lateral)."""
    scissors = []
    for pattern in SCISSOR_PATTERNS:
        value = extract_statistic_value(message, pattern)
        if value:
            scissors.append(f"{pattern}: {value}")
    return ", ".join(scissors) if scissors else ""


def extract_trigram_rolls(message: str) -> str:
    """Extract all roll statistics into one cell."""
    rolls = []
    for pattern in ROLL_PATTERNS:
        value = extract_statistic_value(message, pattern)
        if value:
            # Shorten labels for compactness
            short_label = pattern.replace("2-Roll ", "")
            rolls.append(f"{short_label}: {value}")
    return ", ".join(rolls) if rolls else ""


def extract_trigram_alt(message: str) -> str:
    """Extract Alt percentage from Trigram Statistics."""
    return extract_statistic_value(message, "Alt")


def extract_trigram_redirect(message: str) -> str:
    """Extract Redirect percentage from Trigram Statistics."""
    return extract_statistic_value(message, "Redirect")


def extract_trigram_weak_redirect(message: str) -> str:
    """Extract Weak redirect percentage from Trigram Statistics."""
    return extract_statistic_value(message, "Weak redirect")


def extract_trigram_sfs(message: str) -> str:
    """Extract SFS percentage from Trigram Statistics."""
    return extract_statistic_value(message, "SFS")


# Summary table configuration: (display_header, source_metric, extractor_function)
# extractor_function is None for direct access, or a callable for extracted values
SUMMARY_COLUMNS_CONFIG: list[tuple[str, str, Callable[[str], str] | None]] = [
    ("SVG", "SVG", None),
    ("Total Cost", "Total Cost", None),
    ("Hands Disbalance", "Hands Disbalance", None),
    ("Finger Disbalance", "Finger Disbalance", None),
    ("Alternation", "Trigram Statistics", extract_trigram_alt),
    ("SFB", "Bigram Statistics", extract_bigram_sfb),
    ("Scissors", "Bigram Statistics", extract_bigram_scissors),
    ("2-Rolls", "Trigram Statistics", extract_trigram_rolls),
    ("Redirect", "Trigram Statistics", extract_trigram_redirect),
    ("Weak Redirect", "Trigram Statistics", extract_trigram_weak_redirect),
    ("SFS", "Trigram Statistics", extract_trigram_sfs),
    ("Layout", "Layout", None),
]


def extract_homerow(layout: str) -> str:
    """Extract center keys (homerow) from layout string.

    Layout is 8 clusters of 5 chars + 1 thumb.
    Each cluster: N O C I S (North, Out, Center, In, South)
    Center is at position 2 in each cluster (0-indexed).
    Returns centers from left ring to right ring (6 characters, excluding pinkies).
    """
    if len(layout) < 40:
        return ""

    centers = [layout[cluster_idx * 5 + 2] for cluster_idx in range(8)]
    return "".join(centers)


def build_layout_row(
    layout: str, total_cost: float, metrics_data: dict[str, dict]
) -> dict[str, str | float | int]:
    """Build a dict for one row following COLUMN_HEADERS order."""
    row: dict[str, str | float | int] = {}
    row[COLUMN_HEADERS[0]] = layout  # "Layout"
    row[COLUMN_HEADERS[1]] = extract_homerow(layout)  # "Homerow"

    for display_header, metric_name, format_type, decimals in METRICS_ORDER:
        match format_type:
            case "number" if display_header == "Total Cost":
                row[display_header] = round(total_cost, decimals)
            case "number" if metric_name in metrics_data:
                cost = metrics_data[metric_name]["cost"]
                row[display_header] = round(cost, decimals)
            case "message_only" | "worst_only" if metric_name in metrics_data:
                message = clean_message(
                    metrics_data[metric_name]["message"], metric_name
                )
                row[display_header] = message
            case _:
                row[display_header] = ""
    return row


def parse_layouts(json_file: Path, corpus_name: str | None = None) -> list[dict]:
    """Load results and build a list of dict rows, sorted by total cost."""

    with open(json_file, encoding="utf-8") as f:
        data = json.load(f)

    bigram_frequencies = load_bigram_frequencies(corpus_name) if corpus_name else {}
    sorted_data = sorted(data, key=lambda x: x["total_cost"])

    records: list[dict] = []
    for result in sorted_data:
        layout = result["details"]["layout"]
        total_cost = result["total_cost"]
        metrics_data = process_layout_metrics(result, bigram_frequencies)
        records.append(build_layout_row(layout, total_cost, metrics_data))
    return records


# =============================================================================
# LAYOUT DIAGRAM PARSING AND SVG GENERATION
# =============================================================================


def parse_layout_diagram(text: str) -> list[str]:
    """Parse layout diagram from text and return as list of lines."""
    lines = text.split("\n")
    layout_lines = []

    start_idx = next(
        (i + 1 for i, line in enumerate(lines) if "Layout (layer 1):" in line), None
    )
    if start_idx is None:
        return []

    for line in lines[start_idx:]:
        if "Layout string" in line:
            break
        if line.strip():
            layout_lines.append(line)

    return layout_lines


def export_svg(layout_lines: list[str], output_path: Path) -> None:
    """Create SVG representation of the keyboard layout using Rich."""
    console = Console(record=True, width=64)

    for line in layout_lines:
        styled_line = ""
        for char in line:
            match char:
                case "□":
                    styled_line += f"[gray]{char}[/gray]"
                case _ if char.isalpha():
                    styled_line += f"[yellow]{char}[/yellow]"
                case _:
                    styled_line += char
        console.print(styled_line)

    console.save_svg(output_path, title="", font_aspect_ratio=1)


def parse_diagrams(txt_file: Path, output_dir: Path) -> list[tuple[str, str]]:
    """Parse results.txt file and generate SVG files for each layout."""
    if not txt_file.exists():
        raise FileNotFoundError(f"Results file not found: {txt_file}")

    output_path = Path(output_dir)
    output_path.mkdir(exist_ok=True)

    with open(txt_file, encoding="utf-8") as f:
        content = f.read()

    layout_sections = []
    current_section: list[str] = []

    for line in content.split("\n"):
        if line.startswith("Layout (layer 1):") and current_section:
            layout_sections.append("\n".join(current_section))
            current_section = [line]
        else:
            current_section.append(line)

    if current_section:
        layout_sections.append("\n".join(current_section))

    generated_layouts = []

    for section in layout_sections:
        if not (
            layout_string_match := re.search(
                r"Layout string \(layer 1\):\n(.+)", section
            )
        ):
            continue

        layout_string = layout_string_match.group(1).strip()
        layout_lines = parse_layout_diagram(section)

        if not layout_lines:
            continue

        svg_path = output_path / f"{layout_string}.svg"
        export_svg(layout_lines, svg_path)
        typer.echo(f"Generated: {svg_path}")
        generated_layouts.append((layout_string, str(svg_path)))

    return generated_layouts


# =============================================================================
# MARKDOWN HELPERS
# =============================================================================


def generate_anchor_id(text: str) -> str:
    """Generate a stable anchor ID (ASCII, lowercase). Non-alphanumerics collapse to hyphens."""
    s = unicodedata.normalize("NFKD", text).encode("ascii", "ignore").decode("ascii")
    s = s.lower()
    s = re.sub(r"[^a-z0-9]+", "-", s)  # any run → hyphen
    s = re.sub(r"-{2,}", "-", s).strip("-")
    return s


def _md_cell(v: object) -> str:
    """Escape markdown table delimiters and flatten line breaks."""
    s = "" if v is None else str(v)
    # escape table delimiters and flatten hard line breaks
    s = s.replace("|", r"\|").replace("\n", "<br>")
    return s


# =============================================================================
# COLUMN FILTERING
# =============================================================================


def filter_empty_columns(records: list[dict]) -> list[str]:
    """Return list of column headers that have non-empty values based on first record."""
    if not records:
        return COLUMN_HEADERS

    first_record = records[0]
    non_empty_headers = []
    for header in COLUMN_HEADERS:
        if str(first_record.get(header, "")).strip() != "":
            non_empty_headers.append(header)

    return non_empty_headers


# =============================================================================
# EXPORT FUNCTIONS
# =============================================================================


def export_csv(records: list[dict], output_file: Path) -> None:
    """Export parsed layout records to CSV file."""
    if not records:
        return

    filtered_headers = filter_empty_columns(records)
    with open(output_file, "w", newline="", encoding="utf-8") as f:
        writer = csv.DictWriter(f, fieldnames=filtered_headers, extrasaction="ignore")
        writer.writeheader()
        writer.writerows(records)


def _generate_layout_anchors(records: list[dict]) -> dict[str, str]:
    """Generate stable, unique anchor IDs for all layout headings."""
    used_ids: set[str] = set()
    layout_id: dict[str, str] = {}

    for rec in records:
        base = generate_anchor_id(rec["Layout"])
        anchor = base or "section"
        if anchor in used_ids:
            i = 2
            while f"{anchor}-{i}" in used_ids:
                i += 1
            anchor = f"{anchor}-{i}"
        used_ids.add(anchor)
        layout_id[rec["Layout"]] = anchor

    return layout_id


def _write_table_of_contents(
    f: TextIO, records: list[dict], layout_id: dict[str, str]
) -> None:
    """Write the table of contents section."""
    toc_items = (
        ["- [Summary](#summary)", "- [Layout Details](#layout-details)"]
        + [f"  - [{rec['Layout']}](#{layout_id[rec['Layout']]})" for rec in records]
        + ["- [Metrics Description](#metrics-description)"]
    )
    f.write("## Table of Contents\n\n")
    f.write("\n".join(toc_items) + "\n\n")


def _get_summary_headers(filtered_headers: list[str]) -> list[str]:
    """Determine which summary headers to include based on available data."""
    summary_headers: list[str] = []

    for display_header, source_metric, _ in SUMMARY_COLUMNS_CONFIG:
        # Always include SVG and Layout columns
        if display_header in ["SVG", "Layout"]:
            summary_headers.append(display_header)
        # Include other columns if their source metric exists in data
        elif source_metric in filtered_headers:
            summary_headers.append(display_header)

    return summary_headers


def _build_summary_row_cells(
    rec: dict,
    summary_headers: list[str],
    layout_id: dict[str, str],
    layout_to_svg: dict[str, str],
) -> list[str]:
    """Build row cells for summary table based on headers using configuration."""
    row_cells = []
    layout = rec["Layout"]

    # Build lookup map from display header to config
    config_map = {
        header: (source, extractor_fn)
        for header, source, extractor_fn in SUMMARY_COLUMNS_CONFIG
    }

    for header in summary_headers:
        match header:
            case "SVG":
                svg_cell = (
                    f'<img src="svgs/{quote(Path(layout_to_svg[layout]).name)}" width="1000">'
                    if layout in layout_to_svg
                    else ""
                )
                row_cells.append(_md_cell(svg_cell))
            case "Layout":
                layout_link = f"[{layout}](#{layout_id[layout]})"
                row_cells.append(_md_cell(layout_link))
            case _:
                # Use configuration to determine how to extract the value
                source_metric, extractor_fn = config_map.get(header, (None, None))
                if extractor_fn and source_metric:
                    # Use extractor function to get value from source metric
                    source_data = rec.get(source_metric, "")
                    value = extractor_fn(source_data)
                    # Apply markdown formatting for extracted statistics
                    # Use the header name (not source_metric) for formatting context
                    if header in ["Scissors", "2-Rolls"]:
                        value = format_message_for_markdown(value, header)
                    row_cells.append(_md_cell(value))
                else:
                    # Direct access from record
                    row_cells.append(_md_cell(rec.get(header, "")))

    return row_cells


def _write_summary_table(
    f: TextIO,
    records: list[dict],
    summary_headers: list[str],
    layout_id: dict[str, str],
    generated_layouts: list[tuple[str, str]],
) -> None:
    """Write the summary table section."""
    f.write("## Summary\n\n")

    # Header row
    f.write("| " + " | ".join(summary_headers) + " |\n")
    f.write("|" + "|".join(["--------"] * len(summary_headers)) + "|\n")

    # Map layout -> svg path
    layout_to_svg = dict(generated_layouts) if generated_layouts else {}

    # Data rows
    for rec in records:
        row_cells = _build_summary_row_cells(
            rec, summary_headers, layout_id, layout_to_svg
        )
        f.write("| " + " | ".join(row_cells) + " |\n")

    # Force the table to terminate cleanly in GFM
    f.write("\n<!-- end of summary table -->\n\n")


def _write_layout_details(
    f: TextIO,
    records: list[dict],
    filtered_headers: list[str],
    layout_id: dict[str, str],
    generated_layouts: list[tuple[str, str]],
) -> None:
    """Write the layout details section with individual layout analysis."""
    f.write("## Layout Details\n\n")

    layout_to_svg = dict(generated_layouts) if generated_layouts else {}

    for rec in records:
        layout = rec["Layout"]
        anchor = layout_id[layout]
        f.write(f'<a id="{anchor}"></a>\n')
        f.write(f"### {layout}\n\n")

        # Add SVG image if available
        if layout in layout_to_svg:
            svg_filename = Path(layout_to_svg[layout]).name
            f.write(f'<img src="svgs/{quote(svg_filename)}" width="800">\n\n')

        f.write(f"**Total Cost:** {rec.get('Total Cost', '')}\n\n")

        # Build metrics table excluding Total Cost and any "Worst" summaries
        metrics_data = [
            (header, str(rec.get(header, "")))
            for header in filtered_headers[1:]
            if "Worst" not in header
            and header != "Total Cost"
            and rec.get(header, "") != ""
        ]
        if metrics_data:
            metric_names, values = zip(*metrics_data)
            f.write("| " + " | ".join(metric_names) + " |\n")
            f.write("|" + "|".join(["--------"] * len(metric_names)) + "|\n")
            # Format values for markdown and escape all cell contents to avoid breaking the table
            formatted_values = [
                format_message_for_markdown(v, name)
                for name, v in zip(metric_names, values)
            ]
            f.write("| " + " | ".join(_md_cell(v) for v in formatted_values) + " |\n")

        worst_cases = [
            (header.replace(" Worst", ""), rec.get(header, ""))
            for header in filtered_headers[1:]
            if "Worst" in header and rec.get(header, "")
        ]
        if worst_cases:
            for metric_name, value in worst_cases:
                formatted_value = format_message_for_markdown(value, metric_name)
                f.write(f"- **{metric_name}:** {formatted_value}\n")

        f.write("\n---\n\n")


def export_markdown(
    records: list[dict],
    generated_layouts: list[tuple[str, str]],
    output_file: Path,
) -> None:
    """Export parsed layout records to markdown with summary table and detailed sections.

    Generates a comprehensive markdown report including:
    - Table of contents
    - Summary table with key metrics
    - Detailed layout analysis
    - Metrics descriptions
    """
    filtered_headers = filter_empty_columns(records)
    layout_id = _generate_layout_anchors(records)
    summary_headers = _get_summary_headers(filtered_headers)

    with open(output_file, "w", encoding="utf-8") as f:
        f.write("# Keyboard Layout Results\n\n")

        _write_table_of_contents(f, records, layout_id)
        _write_summary_table(f, records, summary_headers, layout_id, generated_layouts)
        _write_layout_details(
            f, records, filtered_headers, layout_id, generated_layouts
        )

        # ---- Metrics Description ----
        all_headers = set(filtered_headers)
        all_headers.update(summary_headers)
        metrics_description = generate_metrics_description(list(all_headers))
        if metrics_description:
            f.write(f"{metrics_description}")


# =============================================================================
# CLI APPLICATION
# =============================================================================

app = typer.Typer(
    help="Parse keyboard layout optimization results and generate CSV, SVG, and markdown outputs.",
    add_completion=False,
)


@app.command()
def main(
    json_file: Path = typer.Argument(
        ...,
        help="JSON file with optimization results",
        exists=True,
        dir_okay=False,
        readable=True,
    ),
    out: str | None = typer.Option(
        None,
        "--out",
        "-o",
        help="Output directory (default: derived from input file)",
    ),
    corpus: str | None = typer.Option(
        None,
        "--corpus",
        "-c",
        help="Name of the corpus for bigram frequencies",
        callback=lambda x: validate_corpus(x) if x else x,
    ),
) -> None:
    """Parse keyboard layout results and generate outputs. Automatically generates SVG files and markdown table if corresponding .txt file exists."""
    txt_file = json_file.with_suffix(".txt")

    if out:
        output_dir = Path(out)
        output_base = output_dir.name
    else:
        output_base = json_file.stem
        output_dir = json_file.parent / f"{output_base}_layouts"

    output_dir.mkdir(parents=True, exist_ok=True)

    records = parse_layouts(json_file, corpus)

    csv_file = output_dir / f"{output_base}.csv"
    typer.echo(f"Generating CSV: {csv_file}")
    export_csv(records, csv_file)

    if txt_file.exists():
        typer.echo(f"Found {txt_file}, generating SVG files and markdown table...")

        svg_dir = output_dir / "svgs"
        typer.echo(f"Generating SVG files in {svg_dir}...")
        generated_layouts = parse_diagrams(txt_file, svg_dir)

        markdown_file = output_dir / f"{output_base}.md"
        typer.echo(f"Generating markdown table: {markdown_file}")
        export_markdown(records, generated_layouts, markdown_file)


if __name__ == "__main__":
    app()

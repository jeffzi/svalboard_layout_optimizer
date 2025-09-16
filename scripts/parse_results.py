#!/usr/bin/env python3

import csv
import json
import re
from pathlib import Path
from typing import Optional

import typer
from rich.console import Console

# =============================================================================
# CONSTANTS
# =============================================================================

METRICS_ORDER = [
    ("Total Cost", "total_cost", "number", 1),
    ("Hands Disbalance", "Hand Disbalance", "message_only", None),
    ("Finger Disbalance", "Finger Balance", "message_only", None),
    ("Cluster Rolls", "Cluster Rolls", "number", 2),
    ("Scissoring", "Scissoring", "number", 2),
    ("Key Costs", "Key Costs", "number", 2),
    ("Movement Pattern", "Movement Pattern", "number", 2),
    ("Cluster Rolls Worst", "Cluster Rolls", "worst_only", None),
    ("Scissoring Worst", "Scissoring", "worst_only", None),
    ("Movement Pattern Worst", "Movement Pattern", "worst_only", None),
    ("Secondary Bigrams Worst", "Secondary Bigrams", "worst_only", None),
    ("Trigrams Worst", "No Handswitch in Trigram", "worst_only", None),
]

COLUMN_HEADERS = ["Layout"] + [display for display, *_ in METRICS_ORDER]

METRICS_DESCRIPTION = """## Metrics Description

**finger_balance**: Left pinky -> left index and then right index -> right pinky

**hand_disbalance**: Left and right hand balance

**direction_balance**: Tracks keypress patterns in different directions (informational only). Center and south keys are ideal

**key_costs**: Penalizes using keys that are harder to reach based on position (based on direction and finger)

**cluster_rolls**: Evaluates the comfort of same finger bigrams. Center to south bigrams are good here.

**scissoring**: Penalizes uncomfortable adjacent finger movements

**symmetric_handswitches**: Rewards using symmetrical key positions when switching between hands, but only for center, south, and index/middle north keys

**movement_pattern**: Assigns costs to finger transitions within the same hand. If the movement is center key to center key or south key to south key, there is no penalty

**secondary_bigrams**: Evaluates the comfort of the first and last keys in three-key sequences

**no_handswitch_in_trigram**: Penalizes typing three consecutive keys on the same hand

**trigram_rolls**: Rewards comfortable inward rolling motions and slightly less for outward rolls in three-key sequences. Center and south keys only

"""

# =============================================================================
# CORPUS HANDLING
# =============================================================================


def get_corpus_paths(corpus_name: str) -> tuple[Path, Path, Path]:
    """Get corpus directory and ngrams file paths."""
    script_dir = Path(__file__).parent
    project_root = script_dir.parent
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
                if len(bigram) == 2 and bigram.isalpha():
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
            [d.name for d in ngrams_dir.iterdir() if d.is_dir()]
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


def extract_worst_bigrams(message: str) -> list[tuple[str, float]]:
    """Extract bigram pairs from 'Worst:' section of message."""
    if "Worst:" not in message:
        return []

    worst_section = message.split("Worst:")[1]
    if ";" in worst_section:
        worst_section = worst_section.split(";")[0]

    pattern = r"(\w{2}) \(([0-9.]+)%\)"
    matches = re.findall(pattern, worst_section)
    return [(bigram, float(percent)) for bigram, percent in matches]


def add_frequencies(message: str, bigram_frequencies: dict[str, float]) -> str:
    """Enhance Scissoring and Cluster Rolls messages with frequency data."""
    worst_bigrams = extract_worst_bigrams(message)
    if not worst_bigrams or "Worst:" not in message:
        return message

    enhanced_parts = []
    for bigram, percent in worst_bigrams:
        freq = bigram_frequencies.get(bigram, 0)
        freq_str = f"{freq:.2f}%".rstrip("0").rstrip(".")
        enhanced_parts.append(f"{bigram} ({percent}%, freq: {freq_str})")

    if not enhanced_parts:
        return message

    before_worst = message.split("Worst:")[0]
    enhanced_message = f"{before_worst}Worst: {', '.join(enhanced_parts)}"

    if ";" in message:
        after_semicolon = message.split(";", 1)[1]
        enhanced_message += ";" + after_semicolon

    return enhanced_message


def clean_worst_message(message: str, metric_name: str = "") -> str:
    """Clean message by removing 'Worst non-fixed' part and unnecessary prefixes."""
    # Remove ";  Worst non-fixed: ..." part only if it exists (note: two spaces)
    if ";  Worst non-fixed:" in message:
        message = message.split(";  Worst non-fixed:")[0]

    prefixes = ["Finger loads % (no thumb): ", "Hand loads % (no thumb): ", "Worst: "]
    for prefix in prefixes:
        message = message.replace(prefix, "")

    # Format percentages and other numbers
    decimals = 2
    if metric_name in ["Hand Disbalance", "Finger Balance"]:
        decimals = 1
        message = re.sub(
            r"(\d+\.\d+)(?!%\))", lambda m: f"{float(m.group(1)):.{decimals}f}", message
        )
    else:
        decimals = 2
        message = re.sub(
            r"(\d+\.\d+)%,", lambda m: f"{float(m.group(1)):.{decimals}f}%,", message
        )

    return message.strip()


def format_frequencies(message: str) -> str:
    """Format frequencies to 3 decimals while removing trailing zeros."""

    def format_freq(match):
        freq_value = float(match.group(1))
        formatted = f"{freq_value:.3f}".rstrip("0").rstrip(".")
        if "." not in formatted:
            formatted += ".0"
        return f"freq: {formatted}"

    return re.sub(r"freq: (\d+\.?\d*)", format_freq, message)


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

            if core["name"] in ["Scissoring", "Cluster Rolls"] and bigram_frequencies:
                message = add_frequencies(message, bigram_frequencies)
                message = format_frequencies(message)

            metrics_data[core["name"]] = {
                "cost": metric_cost["weighted_cost"],
                "message": message,
            }

    return metrics_data


def build_layout_row(
    layout: str, total_cost: float, metrics_data: dict[str, dict]
) -> dict:
    """Build a dict for one row following COLUMN_HEADERS order.
    Insertion order matches COLUMN_HEADERS.
    """
    row = {}
    row[COLUMN_HEADERS[0]] = layout  # "Layout"

    for display_header, metric_name, format_type, decimals in METRICS_ORDER:
        if format_type == "number" and display_header == "Total Cost":
            row[display_header] = round(total_cost, decimals)
        elif format_type == "number" and metric_name in metrics_data:
            cost = metrics_data[metric_name]["cost"]
            row[display_header] = round(cost, decimals)
        elif (
            format_type in ("message_only", "worst_only")
            and metric_name in metrics_data
        ):
            message = clean_worst_message(
                metrics_data[metric_name]["message"], metric_name
            )
            row[display_header] = message
        else:
            row[display_header] = ""
    return row


def parse_layouts(json_file: Path, corpus_name: Optional[str] = None) -> list[dict]:
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
            if char == "□":
                styled_line += f"[gray]{char}[/gray]"
            elif char.isalpha():
                styled_line += f"[yellow]{char}[/yellow]"
            else:
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
    current_section = []

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
        layout_string_match = re.search(r"Layout string \(layer 1\):\n(.+)", section)
        if not layout_string_match:
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
# EXPORT FUNCTIONS
# =============================================================================


def export_csv(records: list[dict], output_file: Path) -> None:
    """Export parsed layout records to CSV file."""
    with open(output_file, "w", newline="", encoding="utf-8") as f:
        writer = csv.writer(f)
        writer.writerow(COLUMN_HEADERS)
        for rec in records:
            writer.writerow([rec[h] for h in COLUMN_HEADERS])


def export_markdown(
    records: list[dict],
    generated_layouts: list[tuple[str, str]],
    output_file: Path,
) -> None:
    """Export parsed layout records to markdown with summary table and detailed sections."""
    with open(output_file, "w", encoding="utf-8") as f:
        f.write("# Keyboard Layout Results\n\n")

        toc_items = (
            ["- [Summary](#summary)", "- [Layout Details](#layout-details)"]
            + [
                f"  - [{rec['Layout']}](#{rec['Layout'].replace(' ', '_').lower()})"
                for rec in records
            ]
            + ["- [Metrics Description](#metrics-description)"]
        )
        f.write("## Table of Contents\n\n")
        f.write("\n".join(toc_items) + "\n\n")

        f.write("## Summary\n\n")
        summary_headers = [
            "SVG",
            "Total Cost",
            "Hand Balance",
            "Finger Balance",
            "Cluster Rolls",
            "Scissoring",
            "Layout",
        ]
        f.write("| " + " | ".join(summary_headers) + " |\n")
        f.write("|" + "|".join(["--------"] * len(summary_headers)) + "|\n")

        metrics = [
            "Total Cost",
            "Hands Disbalance",
            "Finger Disbalance",
            "Cluster Rolls",
            "Scissoring",
        ]

        layout_to_svg = dict(generated_layouts) if generated_layouts else {}
        for rec in records:
            layout = rec["Layout"]
            svg_cell = (
                f'<img src="svgs/{Path(layout_to_svg[layout]).name}" width="600">'
                if layout in layout_to_svg
                else ""
            )
            layout_link = f"[{layout}](#{layout.replace(' ', '_').lower()})"
            row_cells = (
                [svg_cell]
                + [str(rec.get(metric, "")) for metric in metrics]
                + [layout_link]
            )
            f.write("| " + " | ".join(row_cells) + " |\n")

        f.write("\n## Layout Details\n\n")
        for rec in records:
            layout = rec["Layout"]
            f.write(f"### {layout}\n\n")
            f.write(f"**Total Cost:** {rec.get('Total Cost', '')}\n\n")
            f.write("#### All Metrics\n\n")

            metrics_data = [
                (header, str(rec.get(header, "")))
                for header in COLUMN_HEADERS[1:]
                if "Worst" not in header
                and header != "Total Cost"
                and rec.get(header, "")
            ]
            if metrics_data:
                metric_names, values = zip(*metrics_data)
                f.write("| " + " | ".join(metric_names) + " |\n")
                f.write("|" + "|".join(["--------"] * len(metric_names)) + "|\n")
                f.write("| " + " | ".join(values) + " |\n")

            worst_cases = [
                (header.replace(" Worst", ""), rec.get(header, ""))
                for header in COLUMN_HEADERS[1:]
                if "Worst" in header and rec.get(header, "")
            ]
            if worst_cases:
                f.write("\n#### Worst Cases\n\n")
                for metric_name, value in worst_cases:
                    f.write(f"- **{metric_name}:** {value}\n")

            f.write("\n---\n\n")

        f.write(f"{METRICS_DESCRIPTION}")


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
    out: Optional[str] = typer.Option(
        None,
        "--out",
        "-o",
        help="Output directory (default: derived from input file)",
    ),
    corpus: Optional[str] = typer.Option(
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
        output_dir = Path(f"{output_base}_layouts")

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

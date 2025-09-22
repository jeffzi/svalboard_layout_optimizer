# Svalboard Layout Optimizer

A keyboard layout optimizer forked from [catvw/keyboard_layout_optimizer](https://github.com/catvw/keyboard_layout_optimizer) which added Svalboard support to the original [dariogoetz/keyboard_layout_optimizer](https://github.com/dariogoetz/keyboard_layout_optimizer). This project enhances the optimizer with streamlined workflows, easier layout comparison through CSV and markdown tables, and basic French language support.

## Features

- **Layout Evaluation**: Analyze typing efficiency using various metrics (finger balance, key costs, bigrams, trigrams, scissoring, cluster rolls,etc.)
- **Layout Optimization**: Generate optimal layouts using genetic algorithms or simulated annealing
- **Multi-language Support**: Enhanced n-gram datasets for English, French, and bilingual optimization
- **Svalboard Support**: Built-in support for the [Svalboard](https://svalboard.com/products/lightly) keyboard with custom metrics
- **Streamlined Workflow**: Task automation using Taskfile
- **Flexible Configuration**: Highly customizable metrics and optimization parameters

## Installation

### Prerequisites

Install the required tools:

- **Rust**: Follow the installation guide at [rustup.rs](https://rustup.rs/)
- **Taskfile** (task runner): See installation instructions at [taskfile.dev/installation](https://taskfile.dev/installation/)
- **uv** (Python package manager, for result processing): See installation guide at [docs.astral.sh/uv/getting-started/installation](https://docs.astral.sh/uv/getting-started/installation/)

### Build the Project

```bash
# Clone the repository
git clone https://github.com/jeffzi/svalboard_layout_optimizer
cd svalboard_layout_optimizer

# Build the project
cargo build --release
```

## Quick Start

The project uses [Taskfile](https://taskfile.dev/) for common operations. The main workflows are `optimize` and `evaluate`:

### Optimize Layouts

Generate optimized layouts for a specific language corpus (must be in [ngrams/](ngrams/)):

```bash
# Optimize for English Granite corpus
task optimize CORPUS=eng_granite

# Optimize with custom parameters (fix certain keys)
task optimize CORPUS=eng_fra -- --fix 'reoyaui'

# See optimization options
task optimize CORPUS=eng_fra -- --help
```

### Evaluate Existing Layouts

Evaluate a file of layouts that were previously optimized:

```bash
# Evaluate previously optimized layouts
task evaluate CORPUS=eng_fra

# Evaluate a specific layout file
task evaluate CORPUS=eng_fra LAYOUT_FILE=my_layouts.txt
```

## Output

The `evaluate` task generates comprehensive results in the `evaluation/<corpus>/` directory:

- **CSV file**: Tabulated metrics for easy comparison
- **Markdown report**: Detailed analysis with layout visualizations
- **SVG diagrams**: Visual representations of each layout

The output is processed by [`scripts/parse_results.py`](scripts/parse_results.py) which enhances the raw evaluation data with frequency information and creates user-friendly summaries.

## Language Corpora

The project includes several n-gram datasets in the [`ngrams/`](ngrams/) directory:

### English

- `eng_granite`: [Granite English Ngrams](https://github.com/fohrloop/granite-english-ngrams) corpus
- `eng_web_1m`, `eng_wiki_1m`: Web and Wikipedia corpora

### French

- `fra_news`, `fra_web`, `fra_wikipedia`: Individual French [Leipzig](https://wortschatz.uni-leipzig.de) corpora
- `fra_leipzig`: Combined Leipzig corpora with weighted ratios (web:50, news:30, wikipedia:20)

### Bilingual

- `eng_fra`: English-French bilingual corpus (eng_granite:70, fra_leipzig:30)

All French ngrams were generated using [`scripts/french/Taskfile.yml`](scripts/french/Taskfile.yml).

## Configuration

### Evaluation Metrics

The main metrics configuration is in [`config/evaluation/sval.yml`](config/evaluation/sval.yml). Key metrics include:

- **[finger_balance](config/evaluation/sval.yml#L3)**: Ensures optimal finger load distribution
- **[hand_disbalance](config/evaluation/sval.yml#L24)**: Maintains left-right hand balance
- **[key_costs](config/evaluation/sval.yml#L34)**: Penalizes hard-to-reach keys
- **[cluster_rolls](config/evaluation/sval.yml#L58)**: Evaluates same-finger bigrams comfort
- **[scissoring](config/evaluation/sval.yml#L121)**: Penalizes uncomfortable adjacent finger movements
- **[movement_pattern](config/evaluation/sval.yml#L142)**: Costs finger transitions within the same hand

### Key Costs

Physical key costs are defined in [`config/keyboard/sval.yml`](config/keyboard/sval.yml) under the [`key_costs`](config/keyboard/sval.yml#L162) section. The Svalboard configuration reflects the dual homerow design where:

- **Center & South keys**: Most comfortable (costs: 2-3)
- **Inward keys**: Moderately comfortable (costs: 3-5)
- **Outward keys**: Less comfortable (costs: 4-8)
- **North keys**: Least comfortable (costs: 6-8)

### Svalboard-Specific Metrics

The optimizer includes custom metrics optimized for the Svalboard's unique geometry:

- **[cluster_rolls](config/evaluation/sval.yml#L58)**: Center→South rolls are rewarded (cost: 0.0), other directions penalized appropriately
- **[scissoring](config/evaluation/sval.yml#L121)**: Lateral squeezing motions heavily penalized (cost: 6.0)
- **[movement_pattern](config/evaluation/sval.yml#L142)**: Optimized for the dual-homerow layout with reduced penalties for center-to-center transitions

## Project Structure

```
├── config/
│   ├── evaluation/sval.yml    # Metrics configuration
│   └── keyboard/sval.yml      # Svalboard physical layout
├── ngrams/                    # Language corpora
├── scripts/
│   ├── parse_results.py       # Result processing
│   └── french/Taskfile.yml    # French corpus generation
├── evaluation/                # Generated evaluation results
└── Taskfile.yml              # Main task definitions
```

## Optimization Philosophy

The chosen metric weights aim to produce balanced layouts that:

1. **Respect hand/finger anatomy**: Strong fingers handle more load, weak fingers less
2. **Leverage Svalboard geometry**: Optimize for dual homerows and comfortable key positions
3. **Minimize discomfort**: Heavily penalize scissoring and uncomfortable same-finger sequences
4. **Reward natural motions**: Center→South rolls and smooth finger transitions
5. **Balance typing flow**: Maintain good hand alternation while allowing efficient same-hand patterns

## Advanced Usage

### Direct Binary Usage

For more control, use the compiled binaries directly:

```bash
# Evaluate a specific layout
cargo run --bin evaluate -- \
  --layout-config [config/keyboard/sval.yml](config/keyboard/sval.yml) \
  --ngrams [ngrams/eng_granite](ngrams/eng_granite) \
  "your layout string here"

# Optimize from a starting layout
cargo run --bin optimize_sa -- \
  --layout-config [config/keyboard/sval.yml](config/keyboard/sval.yml) \
  --ngrams [ngrams/eng_fra](ngrams/eng_fra) \
  --start-layouts "starting layout" \
  --append-solutions-to results.txt
```

### Layout String Format

Layouts are specified as space-separated strings representing keys from left to right, top to bottom. Use `□` for placeholder/empty positions:

```
□□gwc□□y□i□□o□u□□e□avxlmh□qnjt□zd□s□bfkpr
```

## Contributing

Contributions are welcome! Areas of particular interest:

- Additional language corpora
- Metric improvements and calibration

## License

This project inherits the GPL-3.0 license from the original keyboard_layout_optimizer.

## Acknowledgments

- [dariogoetz](https://github.com/dariogoetz/keyboard_layout_optimizer) - Original optimizer framework
- [marcusbuffett](https://github.com/marcusbuffett/keyboard_layout_optimizer) - Svalboard metrics inspiration and [optimization insights](https://mbuffett.com/posts/optimizing-datahand-layout/)
- [catvw](https://github.com/catvw/keyboard_layout_optimizer) - Svalboard support and custom metrics implementation
- [Svalboard](https://svalboard.com/products/lightly) - The innovative keyboard this optimizer targets

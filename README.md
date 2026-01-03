# Svalboard Layout Optimizer

A keyboard layout optimizer forked from [catvw/keyboard_layout_optimizer](https://github.com/catvw/keyboard_layout_optimizer) which added Svalboard support to the original [dariogoetz/keyboard_layout_optimizer](https://github.com/dariogoetz/keyboard_layout_optimizer). This project enhances the optimizer with streamlined workflows, easier layout comparison through CSV and markdown tables, and basic French language support.

## Features

- **Layout Evaluation**: Analyze typing efficiency using various metrics (finger balance, key costs, bigrams, trigrams, cost-based scissors, SFB, etc.)
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

# Build the project (optional - cargo run will build automatically)
cargo build --release
```

## Quick Start

**Important**: All commands should be run from the project root directory, not from subdirectories like `ngrams/`.

The project uses [Taskfile](https://taskfile.dev/) to streamline common operations. Taskfile wraps the base CLI commands (see [Advanced Usage](#advanced-usage)) and makes it easier to:

- Manage input/output files with sensible defaults
- Evaluate multiple layouts concurrently
- Generate comprehensive reports (CSV, markdown, SVG) automatically

The main workflows are `optimize` and `evaluate`.

### First Time Setup

Before optimizing, you need a file containing starting layouts (one per line). You can:

1. **Start with a known layout** like QWERTY:

   ```bash
   # Create a starting layouts file
   echo "q□a□zw□sbxe□dtcr□fgvuhj'miyk□,onl□.p-?□□" > eng_shai_layouts.txt
   ```

2. **Use an existing optimized layout** from the community as a starting point

### Complete Workflow Example

```bash
# 1. Create a starting layouts file (replace with your preferred layout)
echo "q□a□zw□sbxe□dtcr□fgvuhj'miyk□,onl□.p-?□□" > eng_shai_layouts.txt

# 2. Run optimization (this will create eng_shai_optimized_layouts.txt)
task optimize CORPUS=eng_shai

# 3. Results are automatically generated in evaluation/eng_shai/
ls evaluation/eng_shai/
```

### Optimize Layouts

Generate optimized layouts for a specific language corpus (must be in [ngrams/](ngrams/)).

**Prerequisites**: You need an input layouts file containing starting layouts (one per line). By default, the task looks for `<CORPUS>_layouts.txt` (e.g., `eng_shai_layouts.txt`).

```bash
# Optimize for English corpus (requires eng_shai_layouts.txt)
task optimize CORPUS=eng_shai

# Use a custom input file
task optimize CORPUS=eng_shai IN_LAYOUT_FILE=my_starts.txt

# Optimize with custom parameters (fix certain keys)
task optimize CORPUS=eng_fra -- --fix 'reoyaui'

# See optimization options
task optimize CORPUS=eng_fra -- --help
```

The optimized layouts will be saved to `<CORPUS>_optimized_layouts.txt` and automatically evaluated.

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

The output is processed by [`scripts/report/report.py`](scripts/report/report.py) which enhances the raw evaluation data with frequency information and creates user-friendly summaries.

## Language Corpora

The project includes several n-gram datasets in the [`ngrams/`](ngrams/) directory:

### English

- `eng_shai`: **[Recommended]** [Shai's Cleaned iweb corpus](https://colemak.com/pub/corpus/iweb-corpus-samples-cleaned.txt.xz) (90M words) - A well-balanced English corpus. Named after Shai Coleman (Colemak creator) who cleaned and published this corpus.
- `eng_web_1m`, `eng_wiki_1m`: Web and Wikipedia corpora

### French

- `fra_news`, `fra_web`, `fra_wikipedia`: Individual French [Leipzig](https://wortschatz.uni-leipzig.de) corpora
- `fra_leipzig`: Combined Leipzig corpora with weighted ratios (web:50, news:30, wikipedia:20)

### Bilingual

- `eng_fra`: English-French bilingual corpus (eng_shai:70, fra_web:30)

All French ngrams were generated using [`scripts/corpora/Taskfile.yml`](scripts/corpora/Taskfile.yml).

## Configuration

### Evaluation Metrics

The main metrics configuration is in [`config/evaluation/sval.yml`](config/evaluation/sval.yml). Key metrics include:

- **finger_balance**: Ensures optimal finger load distribution based on intended loads per finger
- **hand_disbalance**: Maintains left-right hand balance
- **key_costs**: Penalizes hard-to-reach keys based on position difficulty
- **character_constraints**: Applies penalties when specific characters appear at specific positions. Configured here to restrict high-frequency double letters to comfortable positions (center/south)
- **sfb**: Same Finger Bigram metric that evaluates same-finger bigram comfort with directional costs
- **fsb**: Full Scissor Bigram metric that penalizes uncomfortable opposing movements between adjacent fingers (vertical, squeeze, splay)
- **hsb**: Half Scissor Bigram metric that penalizes uncomfortable partial opposing movements between adjacent fingers (diagonal, lateral)
- **manual_bigram_penalty**: Penalizes specific uncomfortable bigrams (e.g., pinky same-key repeats)
- **sympathetic**: Penalizes adjacent finger bigrams where fingers move in different directions. Same-direction movements benefit from finger coupling (enslaving), while different directions create conflict
- **bigram_stats**: Provides statistics on bigram categories like SFB, scissor types, and other movement patterns. Supports `ignore_movements` to exclude specific direction pairs (e.g., Center→South) from SFB count (informational, weight: 0)
- **trigram_stats**: Tracks roll and redirect statistics. Supports `same_finger_rolls` to track specific same-finger movements (e.g., Center→South, In→South) separately within bigram rolls (informational, weight: 0)

### Key Costs

Physical key costs are defined in [`config/keyboard/sval.yml`](config/keyboard/sval.yml) under the `key_costs` section. The Svalboard configuration reflects the dual homerow design where:

- **Center & South keys**: Most comfortable
- **Inward keys**: Moderately comfortable
- **Outward keys**: Less comfortable
- **North keys**: Least comfortable

### Svalboard-Specific Metrics

The optimizer includes custom metrics optimized for the Svalboard's unique geometry:

- **sfb**: Same Finger Bigram metric with directional costs:

  - Center→South movements are rewarded
  - Other directions penalized based on comfort
  - Finger multipliers increase penalties for weaker fingers
  - High-frequency SFBs get additional penalty multiplier

- **fsb**: Full Scissor Bigram metric that penalizes uncomfortable opposing movements between adjacent fingers based on inherent biomechanical discomfort:

  - **Vertical**: Opposite vertical directions (North ↔ South)
  - **Squeeze**: Fingers moving toward each other (In ↔ Out, inward motion - more uncomfortable)
  - **Splay**: Fingers moving apart (In ↔ Out, outward motion - less uncomfortable)
  - Each movement type has configurable base costs
  - Optional finger multipliers (weaker fingers dominate)
  - Optional high-frequency bigram penalty multiplier

- **hsb**: Half Scissor Bigram metric that penalizes uncomfortable partial opposing movements between adjacent fingers:

  - **Diagonal**: Lateral + Vertical movements (one finger moves laterally In/Out, other vertically North/South)
  - **Lateral**: Lateral + Center movements (one finger moves laterally In/Out, other presses Center)
  - Each movement type has configurable base costs
  - Optional finger multipliers (weaker fingers dominate)
  - Optional high-frequency bigram penalty multiplier

- **sympathetic**: Penalizes adjacent finger bigrams where fingers move in different directions:

  - Same-direction movements (e.g., South→South) benefit from finger coupling—enslaving helps
  - Different directions create conflict—the second finger fights involuntary force
  - Finger-pair coupling: Ring-Pinky > Middle-Ring > Index-Middle
  - Center (rest position) is ignored since it's not an actual movement

- **character_constraints**: Penalizes specific characters at specific matrix positions. Currently configured to:
  - Restrict common double letters (e, l, s, o, t, r, h, n, f, p) to comfortable positions (center/south preferred)
  - Keep punctuation marks (,.'- ) off center keys to preserve homerow flow
  - This metric is highly customizable for enforcing character placement constraints

## Project Structure

```
├── config/
│   ├── evaluation/sval.yml    # Metrics configuration
│   └── keyboard/sval.yml      # Svalboard physical layout
├── ngrams/                    # Language corpora
├── scripts/
│   ├── report/report.py       # Result processing
│   └── corpora/Taskfile.yml   # Corpus generation workflows
├── evaluation/                # Generated evaluation results
└── Taskfile.yml              # Main task definitions
```

## Optimization Philosophy

The chosen metric weights aim to produce balanced layouts that:

1. **Respect hand/finger anatomy**: Strong fingers handle more load, weak fingers less
2. **Leverage Svalboard geometry**: Optimize for dual homerows and comfortable key positions
3. **Minimize discomfort**: Cost-based penalties for scissors (effort imbalances between adjacent fingers) and uncomfortable same-finger bigrams
4. **Reward natural motions**: Center→South movements and smooth finger transitions
5. **Balance typing flow**: Maintain good hand alternation while allowing efficient same-hand patterns

## Recommended Optimization Workflow

The optimizer can get stuck in local minima, so starting from well-performing layouts yields better results than random initialization. Here's a proven iterative approach:

1. **Gather established layouts**: Create a file with modern, well-designed layouts (one per line):

   ```bash
   # Add layouts like Hands Down variants or other proven designs
   echo "'□cqb-□i□y□?e□o□.a,um□hklgjt□dwxn□pvzs□fr" > layouts.txt
   echo "your_other_layout_here" >> layouts.txt
   ```

2. **Run optimization with fixed clusters**: Lock the vowel and consonant clusters that form the backbone of modern layouts:

   ```bash
   task optimize CORPUS=eng_shai IN_LAYOUT_FILE=layouts.txt -- --fix 'yiouearsnth'
   ```

   This keeps proven letter groups (vowels + high-frequency consonants like 'snth') in place while optimizing around them.

3. **Compare results**: Review the generated report in `evaluation/eng_shai/`:

   - Check the CSV for metric comparisons
   - Review the markdown report for detailed analysis
   - Look for layouts that score well across multiple metrics

4. **Refine metrics if needed**: If layouts you know are good score poorly:

   - Adjust metric weights and parameters in `config/evaluation/sval.yml`
   - Adjust key costs in `config/keyboard/sval.yml` (rarely needed - only affects `key_costs` metric)
   - Re-evaluate to verify improvements

5. **Iterate**: Keep top performers, add them to your layouts file, and repeat

**Escaping local minima**: If you want to explore variations of a specific layout, make small manual tweaks (swap 1-2 letters) and lock them:

```bash
# Direct binary usage for fine-grained control
N_WORST=5 SHOW_WORST=true cargo run --bin optimize_genetic -- \
  --layout-config config/keyboard/sval.yml \
  --ngrams ngrams/eng_shai \
  --fix "ioueansthy" \
  --start-layout "□□y□i,□o□u□□e□a.-h'lk?tmgxjnqdzbs□f□wcvpr"
```

This approach adapts proven split-keyboard principles to Svalboard geometry rather than trying to reinvent layouts from scratch.

## Advanced Usage

### Direct Binary Usage

For more control or integration into custom workflows, you can use the compiled binaries directly instead of Taskfile.

**Important**: Always use `--release` flag for optimized performance (faster than debug builds):

```bash
# Evaluate a specific layout
cargo run --release --bin evaluate -- \
  --layout-config config/keyboard/sval.yml \
  --ngrams ngrams/eng_shai \
  "your layout string here"

# Optimize from a starting layout
cargo run --release --bin optimize_sa -- \
  --layout-config config/keyboard/sval.yml \
  --ngrams ngrams/eng_shai \
  --start-layouts "starting layout" \
  --append-solutions-to results.txt
```

### Layout String Format

Layouts are continuous strings where:

1. Each finger cluster has 5 ordered keys (north, west, center, east, south)
2. The final character is the alpha thumb key if defined
3. Use `□` for placeholder/empty positions

Hands Down Promethium (mirrored):

```
'□cqb-□i□y□?e□o□.a,um□hklgjt□dwxn□pvzs□fr
```

## Contributing

Contributions are welcome! Areas of particular interest:

- Metric improvements and calibration

## License

This project inherits the GPL-3.0 license from the original keyboard_layout_optimizer.

## Troubleshooting

Make sure you're in the project root directory (where `Taskfile.yml` is located), not in subdirectories like `ngrams/`.

### "Error: Input layouts file '...\_layouts.txt' not found"

This means you need to create a starting layouts file before running optimization.

The default input filename follows the pattern: `<CORPUS>_layouts.txt`

- For `CORPUS=eng_shai`, it expects `eng_shai_layouts.txt`
- For `CORPUS=eng_fra`, it expects `eng_fra_layouts.txt`

**Solution**:

```bash
# Create a layouts file with a starting layout (filename must match corpus name)
echo "'□cqb-□i□y□?e□o□.a,um□hklgjt□dwxn□pvzs□fr" > eng_shai_layouts.txt

# Or specify a different file
task optimize CORPUS=eng_shai IN_LAYOUT_FILE=my_layouts.txt
```

## Acknowledgments

- [dariogoetz](https://github.com/dariogoetz/keyboard_layout_optimizer) - Original optimizer framework
- [marcusbuffett](https://github.com/marcusbuffett/keyboard_layout_optimizer) - Svalboard metrics inspiration and [optimization insights](https://mbuffett.com/posts/optimizing-datahand-layout/)
- [catvw](https://github.com/catvw/keyboard_layout_optimizer) - Svalboard support and custom metrics implementation
- [Svalboard](https://svalboard.com/products/lightly) - The innovative keyboard this optimizer targets

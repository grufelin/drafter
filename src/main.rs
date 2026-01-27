use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use anyhow::{anyhow, Context, Result};
use clap::{Args, Parser, Subcommand, ValueEnum};
use rand::rngs::StdRng;
use rand::SeedableRng;
use serde::{Deserialize, Serialize};

use drafter::llm::PhraseAlternative;
use drafter::planner::{generate_plan, generate_plan_with_phrase_alternatives, PlannerConfig};
use drafter::playback::play_plan;
use drafter::sim;
use drafter::word_nav_profile::WordNavProfile;

const DEFAULT_LLM_MODEL: &str = drafter::llm::openrouter::DEFAULT_MODEL;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum LlmRewriteStrengthArg {
    Subtle,
    Moderate,
    Dramatic,
}

impl LlmRewriteStrengthArg {
    fn to_library(self) -> drafter::llm::RewriteStrength {
        match self {
            LlmRewriteStrengthArg::Subtle => drafter::llm::RewriteStrength::Subtle,
            LlmRewriteStrengthArg::Moderate => drafter::llm::RewriteStrength::Moderate,
            LlmRewriteStrengthArg::Dramatic => drafter::llm::RewriteStrength::Dramatic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum LlmFailurePolicy {
    /// On any LLM/cache error, fall back to non-LLM planning.
    Fallback,
    /// On any LLM/cache error, return an error.
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum PlaybackBackendArg {
    Auto,
    Wayland,
    X11,
}

impl PlaybackBackendArg {
    fn to_library(self) -> drafter::playback::PlaybackBackend {
        match self {
            PlaybackBackendArg::Auto => drafter::playback::PlaybackBackend::Auto,
            PlaybackBackendArg::Wayland => drafter::playback::PlaybackBackend::Wayland,
            PlaybackBackendArg::X11 => drafter::playback::PlaybackBackend::X11,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum WordNavProfileArg {
    /// Current behavior; best for Chrome/Docs-like editors.
    Chrome,
    /// Conservative mode: fewer Ctrl+word shortcuts; more robust across toolkits.
    Compatible,
}

impl WordNavProfileArg {
    fn to_library(self) -> WordNavProfile {
        match self {
            WordNavProfileArg::Chrome => WordNavProfile::Chrome,
            WordNavProfileArg::Compatible => WordNavProfile::Compatible,
        }
    }
}

#[derive(Debug, Args, Clone)]
struct LlmArgs {
    /// Enable paragraph-level phrase alternatives via OpenRouter.
    ///
    /// Requires `--features llm` unless you provide an existing `--llm-cache` file.
    #[arg(long)]
    llm: bool,

    /// OpenRouter model name.
    #[arg(long, default_value_t = DEFAULT_LLM_MODEL.to_string(), requires = "llm")]
    llm_model: String,

    /// Maximum suggestions per paragraph.
    #[arg(long, default_value_t = 4, requires = "llm")]
    llm_max_suggestions: usize,

    /// How strongly to rewrite suggested alternatives.
    #[arg(long, value_enum, default_value_t = LlmRewriteStrengthArg::Subtle, requires = "llm")]
    llm_rewrite_strength: LlmRewriteStrengthArg,

    /// Maximum number of concurrent OpenRouter requests.
    #[arg(
        long,
        default_value_t = 10,
        requires = "llm",
        value_parser = clap::value_parser!(u8).range(1..=10)
    )]
    llm_max_concurrency: u8,

    /// Optional JSON cache path for LLM suggestions.
    ///
    /// If the file exists, it is used and no network requests are made.
    /// If the file does not exist, it is written after a successful fetch.
    /// Note: the cache contains your draft text.
    #[arg(long, value_name = "PATH", requires = "llm")]
    llm_cache: Option<PathBuf>,

    /// What to do if any LLM request or cache load fails.
    #[arg(long, value_enum, default_value_t = LlmFailurePolicy::Fallback, requires = "llm")]
    llm_on_error: LlmFailurePolicy,
}

#[derive(Debug, Parser)]
#[command(name = "drafter")]
#[command(about = "Human-like typing simulator for Wayland and X11 editors", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Generate a typing plan (JSON)
    Plan {
        /// Input text file, or '-' for stdin
        #[arg(long, value_name = "PATH")]
        input: PathBuf,

        /// Output plan file (defaults to stdout)
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,

        /// Optional RNG seed (for debugging)
        #[arg(long)]
        seed: Option<u64>,

        #[arg(long, default_value_t = 80.0)]
        wpm_min: f64,

        #[arg(long, default_value_t = 120.0)]
        wpm_max: f64,

        /// Error probability per word (0.0-1.0).
        ///
        /// Set to 0 for straight-through typing (no revisions/corrections).
        #[arg(long, default_value_t = 0.05)]
        error_rate: f64,

        /// Immediate fix probability when an error is made (0.0-1.0)
        #[arg(long, default_value_t = 0.35)]
        immediate_fix_rate: f64,

        /// Word navigation profile for Ctrl+Left/Right during corrections.
        ///
        /// - chrome: current behavior; best for Chrome/Docs-like editors.
        /// - compatible: fewer Ctrl+word shortcuts; more robust across toolkits.
        #[arg(long, value_enum, default_value_t = WordNavProfileArg::Compatible)]
        profile: WordNavProfileArg,

        #[command(flatten)]
        llm: LlmArgs,
    },

    /// Play a plan into the currently focused editor
    Play {
        /// Playback backend.
        ///
        /// - auto: choose a backend based on the runtime environment
        /// - wayland: force Wayland playback
        /// - x11: force X11 playback (XTEST)
        #[arg(long, value_enum, default_value_t = PlaybackBackendArg::Auto)]
        backend: PlaybackBackendArg,

        /// Plan file (JSON)
        #[arg(long, value_name = "PATH")]
        plan: PathBuf,

        /// Countdown seconds before playback starts
        #[arg(long, default_value_t = 5)]
        countdown: u64,

        /// Wayland seat name to attach the virtual keyboard to (e.g. seat0, seat1).
        #[arg(long, value_name = "NAME")]
        seat: Option<String>,

        /// Disable console typing trace output
        #[arg(long)]
        no_trace: bool,
    },

    /// Generate a plan then immediately play it
    Run {
        /// Playback backend.
        ///
        /// - auto: choose a backend based on the runtime environment
        /// - wayland: force Wayland playback
        /// - x11: force X11 playback (XTEST)
        #[arg(long, value_enum, default_value_t = PlaybackBackendArg::Auto)]
        backend: PlaybackBackendArg,

        /// Input text file, or '-' for stdin
        #[arg(long, value_name = "PATH")]
        input: PathBuf,

        /// Countdown seconds before playback starts
        #[arg(long, default_value_t = 5)]
        countdown: u64,

        /// Wayland seat name to attach the virtual keyboard to (e.g. seat0, seat1).
        #[arg(long, value_name = "NAME")]
        seat: Option<String>,

        /// Disable console typing trace output
        #[arg(long)]
        no_trace: bool,

        /// Optional output plan file to save
        #[arg(long, value_name = "PATH")]
        output: Option<PathBuf>,

        /// Optional RNG seed (for debugging)
        #[arg(long)]
        seed: Option<u64>,

        #[arg(long, default_value_t = 80.0)]
        wpm_min: f64,

        #[arg(long, default_value_t = 120.0)]
        wpm_max: f64,

        /// Error probability per word (0.0-1.0).
        ///
        /// Set to 0 for straight-through typing (no revisions/corrections).
        #[arg(long, default_value_t = 0.05)]
        error_rate: f64,

        /// Immediate fix probability when an error is made (0.0-1.0)
        #[arg(long, default_value_t = 0.35)]
        immediate_fix_rate: f64,

        /// Word navigation profile for Ctrl+Left/Right during corrections.
        ///
        /// - chrome: current behavior; best for Chrome/Docs-like editors.
        /// - compatible: fewer Ctrl+word shortcuts; more robust across toolkits.
        #[arg(long, value_enum, default_value_t = WordNavProfileArg::Compatible)]
        profile: WordNavProfileArg,

        #[command(flatten)]
        llm: LlmArgs,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LlmCacheFile {
    version: u32,
    model: String,
    max_suggestions: usize,
    rewrite_strength: LlmRewriteStrengthArg,
    paragraphs: Vec<String>,
    alternatives_by_paragraph: Vec<Vec<PhraseAlternative>>,
}

fn read_input(path: &PathBuf) -> Result<String> {
    if path.as_os_str() == std::ffi::OsStr::new("-") {
        let mut buf = String::new();
        io::stdin()
            .read_to_string(&mut buf)
            .context("failed to read stdin")?;
        return Ok(buf);
    }

    fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))
}

fn write_output(path: &PathBuf, contents: &str) -> Result<()> {
    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))
}

fn build_config(
    wpm_min: f64,
    wpm_max: f64,
    error_rate: f64,
    immediate_fix_rate: f64,
    profile: WordNavProfileArg,
) -> PlannerConfig {
    PlannerConfig {
        wpm_min,
        wpm_max,
        error_rate_per_word: error_rate,
        immediate_fix_rate,
        word_nav_profile: profile.to_library(),
        ..Default::default()
    }
}

fn rng_from_seed(seed: Option<u64>) -> StdRng {
    match seed {
        Some(seed) => StdRng::seed_from_u64(seed),
        None => StdRng::from_entropy(),
    }
}

fn split_into_non_empty_paragraphs(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let len = bytes.len();
    let mut paragraphs = Vec::new();
    let mut idx = 0usize;

    while idx < len {
        while idx < len && bytes[idx] == b'\n' {
            idx += 1;
        }
        if idx >= len {
            break;
        }

        let start = idx;
        while idx < len {
            if bytes[idx] == b'\n' && idx + 1 < len && bytes[idx + 1] == b'\n' {
                break;
            }
            idx += 1;
        }
        let end = idx;
        paragraphs.push(text[start..end].to_string());

        while idx < len && bytes[idx] == b'\n' {
            idx += 1;
        }
    }

    paragraphs
}

fn maybe_generate_plan(
    final_text: &str,
    cfg: PlannerConfig,
    llm: &LlmArgs,
    rng: &mut StdRng,
) -> Result<drafter::model::Plan> {
    if llm.llm && cfg.error_rate_per_word == 0.0 {
        return Err(anyhow!(
            "--llm is incompatible with --error-rate 0 (no-revision mode)"
        ));
    }

    if !llm.llm {
        return generate_plan(final_text, cfg, rng);
    }

    let paragraphs = split_into_non_empty_paragraphs(final_text);
    if paragraphs.is_empty() || llm.llm_max_suggestions == 0 {
        return generate_plan(final_text, cfg, rng);
    }

    let has_existing_cache = llm.llm_cache.as_ref().map(|p| p.exists()).unwrap_or(false);
    if !has_existing_cache && !cfg!(feature = "llm") {
        return Err(anyhow!(
            "LLM support is disabled (build with --features llm), or provide an existing --llm-cache file"
        ));
    }

    let alternatives_by_paragraph = match load_or_fetch_llm_suggestions(&paragraphs, llm) {
        Ok(items) => items,
        Err(err) => match llm.llm_on_error {
            LlmFailurePolicy::Fallback => {
                eprintln!("LLM suggestions unavailable ({err:#}). Falling back to non-LLM plan.");
                return generate_plan(final_text, cfg, rng);
            }
            LlmFailurePolicy::Error => return Err(err),
        },
    };

    generate_plan_with_phrase_alternatives(final_text, cfg, &alternatives_by_paragraph, rng)
}

fn load_or_fetch_llm_suggestions(
    paragraphs: &[String],
    llm: &LlmArgs,
) -> Result<Vec<Vec<PhraseAlternative>>> {
    if let Some(cache_path) = &llm.llm_cache {
        if cache_path.exists() {
            let cached = load_llm_cache(cache_path)?;
            if cached.paragraphs != paragraphs {
                return Err(anyhow!(
                    "LLM cache paragraphs do not match input; delete the cache or choose another path"
                ));
            }
            return Ok(cached.alternatives_by_paragraph);
        }
    }

    let options = drafter::llm::ParagraphRephraseOptions {
        max_suggestions: llm.llm_max_suggestions,
        strength: llm.llm_rewrite_strength.to_library(),
    };

    let fetched = fetch_openrouter_suggestions(llm, paragraphs, options)?;

    if let Some(cache_path) = &llm.llm_cache {
        let cache = LlmCacheFile {
            version: 1,
            model: llm.llm_model.clone(),
            max_suggestions: llm.llm_max_suggestions,
            rewrite_strength: llm.llm_rewrite_strength,
            paragraphs: paragraphs.to_vec(),
            alternatives_by_paragraph: fetched.clone(),
        };
        write_llm_cache(cache_path, &cache)?;
    }

    Ok(fetched)
}

fn load_llm_cache(path: &PathBuf) -> Result<LlmCacheFile> {
    let json =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    let cache: LlmCacheFile =
        serde_json::from_str(&json).context("failed to parse LLM cache JSON")?;

    if cache.version != 1 {
        return Err(anyhow!(
            "unsupported LLM cache version {}; expected 1",
            cache.version
        ));
    }

    Ok(cache)
}

fn write_llm_cache(path: &PathBuf, cache: &LlmCacheFile) -> Result<()> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .with_context(|| format!("failed to create directory {}", parent.display()))?;
    }

    let json = serde_json::to_string_pretty(cache).context("failed to serialize LLM cache")?;
    write_output(path, &json)
}

#[cfg(feature = "llm")]
fn fetch_openrouter_suggestions(
    llm: &LlmArgs,
    paragraphs: &[String],
    options: drafter::llm::ParagraphRephraseOptions,
) -> Result<Vec<Vec<PhraseAlternative>>> {
    use drafter::llm::openrouter::OpenRouterParagraphRephraseClient;

    let runtime = tokio::runtime::Runtime::new().context("failed to start tokio runtime")?;
    runtime.block_on(async {
        let client = OpenRouterParagraphRephraseClient::from_env()?
            .with_model(llm.llm_model.clone())
            .with_max_concurrency(llm.llm_max_concurrency as usize);

        client
            .rephrase_paragraphs(paragraphs, options)
            .await
            .context("OpenRouter rephrase_paragraphs failed")
    })
}

#[cfg(not(feature = "llm"))]
fn fetch_openrouter_suggestions(
    _llm: &LlmArgs,
    _paragraphs: &[String],
    _options: drafter::llm::ParagraphRephraseOptions,
) -> Result<Vec<Vec<PhraseAlternative>>> {
    Err(anyhow!(
        "LLM support is disabled (build with --features llm), or provide an existing --llm-cache file"
    ))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Plan {
            input,
            output,
            seed,
            wpm_min,
            wpm_max,
            error_rate,
            immediate_fix_rate,
            profile,
            llm,
        } => {
            let final_text = read_input(&input)?;
            let cfg = build_config(wpm_min, wpm_max, error_rate, immediate_fix_rate, profile);
            let mut rng = rng_from_seed(seed);

            let plan = maybe_generate_plan(&final_text, cfg, &llm, &mut rng)?;

            let stats = sim::stats(&plan);
            eprintln!(
                "Planned: {} actions, {} key events, ~{:.1} min, target {:.1} WPM",
                stats.actions,
                stats.key_events,
                (stats.total_wait_ms as f64) / 1000.0 / 60.0,
                plan.config.wpm_target
            );

            let json = serde_json::to_string_pretty(&plan).context("failed to serialize plan")?;
            if let Some(out) = output {
                write_output(&out, &json)?;
            } else {
                println!("{json}");
            }
        }
        Command::Play {
            plan,
            countdown,
            backend,
            seat,
            no_trace,
        } => {
            // Fail fast on unsupported environments/backends.
            drafter::playback::resolve_backend(backend.to_library())?;

            let json = fs::read_to_string(&plan)
                .with_context(|| format!("failed to read {}", plan.display()))?;
            let plan: drafter::model::Plan =
                serde_json::from_str(&json).context("failed to parse plan JSON")?;

            let stats = sim::stats(&plan);
            eprintln!(
                "Playing: {} actions, {} key events, ~{:.1} min",
                stats.actions,
                stats.key_events,
                (stats.total_wait_ms as f64) / 1000.0 / 60.0
            );

            play_plan(
                &plan,
                countdown,
                !no_trace,
                seat.as_deref(),
                backend.to_library(),
            )?;
        }
        Command::Run {
            input,
            countdown,
            backend,
            seat,
            no_trace,
            output,
            seed,
            wpm_min,
            wpm_max,
            error_rate,
            immediate_fix_rate,
            profile,
            llm,
        } => {
            // Fail fast on unsupported environments/backends.
            drafter::playback::resolve_backend(backend.to_library())?;

            let final_text = read_input(&input)?;
            let cfg = build_config(wpm_min, wpm_max, error_rate, immediate_fix_rate, profile);
            let mut rng = rng_from_seed(seed);

            let plan = maybe_generate_plan(&final_text, cfg, &llm, &mut rng)?;

            let stats = sim::stats(&plan);
            eprintln!(
                "Planned: {} actions, {} key events, ~{:.1} min, target {:.1} WPM",
                stats.actions,
                stats.key_events,
                (stats.total_wait_ms as f64) / 1000.0 / 60.0,
                plan.config.wpm_target
            );

            if let Some(out) = output {
                let json =
                    serde_json::to_string_pretty(&plan).context("failed to serialize plan")?;
                write_output(&out, &json)?;
            }

            play_plan(
                &plan,
                countdown,
                !no_trace,
                seat.as_deref(),
                backend.to_library(),
            )?;
        }
    }

    Ok(())
}

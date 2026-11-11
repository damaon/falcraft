mod anim;
mod audio;
mod export;
mod llm;
mod render;
mod short;
mod text;
mod video;
mod voxels;

use anyhow::{bail, Context, Result};
use clap::{Args, Parser, Subcommand};
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(about = "Satisfying voxel build shorts (9:16), incl. AI build battles")]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Render one or more build JSON files into a short (several files = battle layout).
    Render {
        specs: Vec<PathBuf>,
        #[arg(short, long, default_value = "out/build.mp4")]
        out: PathBuf,
        /// Big caption; defaults to the spec title.
        #[arg(long)]
        title: Option<String>,
        #[command(flatten)]
        video: VideoArgs,
    },
    /// Ask several LLMs to build the same prompt and render the battle short.
    Battle {
        prompt: String,
        /// OpenRouter model ids, comma separated; "id=Display Name" to rename.
        #[arg(long, value_delimiter = ',', default_value = "anthropic/claude-opus-5.5,openai/gpt-6.1-sol,google/gemini-4-argon")]
        models: Vec<String>,
        /// Directory for generated specs, raw responses and the video.
        #[arg(long, default_value = "out")]
        out: PathBuf,
        /// Max grid edge the models may use.
        #[arg(long, default_value_t = 48)]
        grid: i32,
        /// Re-render from previously generated specs instead of calling the models again.
        #[arg(long)]
        reuse: bool,
        /// Reveal which model made which build in the final lineup (off: kid-friendly, no AI mention).
        #[arg(long)]
        reveal_names: bool,
        #[command(flatten)]
        video: VideoArgs,
    },
    /// Minecraft downloads (Java world zip + Bedrock .mcworld) for every builds/<slug>/<n>.json.
    Export {
        /// Folder with one sub-folder per battle.
        #[arg(default_value = "builds")]
        builds: PathBuf,
        #[arg(short, long, default_value = "web/static/downloads")]
        out: PathBuf,
    },
    /// Print the system prompt the models get (handy for testing the DSL by hand).
    Prompt {
        #[arg(long, default_value_t = 48)]
        grid: i32,
    },
}

#[derive(Args)]
struct VideoArgs {
    #[arg(long, default_value_t = 60)]
    fps: u32,
    /// Output height in pixels (width is 9:16 of it). 960 is a quick preview.
    #[arg(long, default_value_t = 1920)]
    height: u32,
    /// Seconds each build takes to assemble.
    #[arg(long, default_value_t = 6.5)]
    build_secs: f32,
    /// Seconds to admire a finished build before the next one.
    #[arg(long, default_value_t = 1.6)]
    hold_secs: f32,
    #[arg(long, default_value_t = 5.0)]
    lineup_secs: f32,
    /// Opening hook showing the finished builds (battles only); 0 disables.
    #[arg(long, default_value_t = 1.5)]
    teaser_secs: f32,
    /// day | candy | sunset | night | moon | mint
    #[arg(long, default_value = "day")]
    theme: String,
    #[arg(long)]
    font: Option<String>,
    /// Render a single PNG at this time (seconds) instead of the video.
    #[arg(long)]
    still: Option<f32>,
}

fn main() -> Result<()> {
    match Cli::parse().cmd {
        Cmd::Render { specs, out, title, video } => {
            if specs.is_empty() {
                bail!("pass at least one build JSON");
            }
            let mut entries = Vec::new();
            let mut first_title = String::new();
            for (i, path) in specs.iter().enumerate() {
                let spec = load_spec(path)?;
                if first_title.is_empty() {
                    first_title = spec.title.clone();
                }
                let alias = if specs.len() > 1 { format!("#{}", i + 1) } else { String::new() };
                entries.push(entry(&spec, alias, String::new(), &video)?);
            }
            let battle = entries.len() > 1;
            let cfg = config(
                &video,
                if battle { "BUILD BATTLE" } else { "" },
                title.unwrap_or(first_title).to_uppercase(),
            );
            produce(cfg, entries, &video, &out)
        }
        Cmd::Battle { prompt, models, out, grid, reuse, reveal_names, video } => {
            let dir = out.join(slug(&prompt));
            std::fs::create_dir_all(&dir)?;
            let specs = generate_all(&prompt, &models, grid, &dir, reuse)?;
            // Deterministic shuffle so the slot a model lands in doesn't give it away.
            let mut order: Vec<usize> = (0..specs.len()).collect();
            let seed = prompt.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
            order.sort_by_key(|i| splitmix(seed ^ *i as u64));
            let mut entries = Vec::new();
            for (slot, &i) in order.iter().enumerate() {
                let (name, spec) = &specs[i];
                let shown = if reveal_names { name.clone() } else { String::new() };
                entries.push(entry(spec, format!("#{}", slot + 1), shown, &video)?);
            }
            let cfg = config(&video, "BUILD BATTLE", prompt.to_uppercase());
            produce(cfg, entries, &video, &dir.join("battle.mp4"))
        }
        Cmd::Export { builds, out } => export_all(&builds, &out),
        Cmd::Prompt { grid } => {
            println!("{}", llm::system_prompt(grid));
            Ok(())
        }
    }
}

fn load_spec(path: &Path) -> Result<voxels::BuildSpec> {
    let text = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    voxels::BuildSpec::from_llm_text(&text).with_context(|| format!("parse {}", path.display()))
}

fn entry(spec: &voxels::BuildSpec, alias: String, name: String, v: &VideoArgs) -> Result<short::Entry> {
    let grid = spec.voxelize()?;
    eprintln!("  {alias} {name}: {} voxels ({} visible), grid {:?}", grid.count(), grid.surface_count(), grid.size.to_array());
    Ok(short::Entry { alias, name, anim: anim::BuildAnim::new(&grid, v.build_secs) })
}

fn config(v: &VideoArgs, kicker: &str, title: String) -> short::Config {
    let height = v.height - v.height % 16;
    let width = (height * 9 / 16) & !1;
    short::Config {
        width,
        height,
        fps: v.fps,
        build_secs: v.build_secs,
        hold_secs: v.hold_secs,
        lineup_secs: v.lineup_secs,
        kicker: kicker.to_string(),
        title,
        lineup_question: "WHICH ONE IS THE BEST?".into(),
        theme: render::Theme::by_name(&v.theme),
        teaser_secs: v.teaser_secs,
    }
}

fn produce(cfg: short::Config, entries: Vec<short::Entry>, v: &VideoArgs, out: &Path) -> Result<()> {
    if let Some(parent) = out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut renderer = render::Renderer::new(cfg.width, cfg.height, cfg.theme)?;
    let mut text = text::TextCache::new(text::load_font(v.font.as_deref())?);
    let sh = short::Short::new(cfg, entries);
    match v.still {
        Some(t) => {
            let png = out.with_extension(format!("{t:.1}s.png"));
            sh.render_still(&mut renderer, &mut text, t, &png)?;
            eprintln!("  still -> {}", png.display());
        }
        None => {
            eprintln!("  rendering {:.1}s short", sh.duration);
            sh.render_video(&mut renderer, &mut text, out)?;
        }
    }
    Ok(())
}

fn generate_all(prompt: &str, models: &[String], grid: i32, dir: &Path, reuse: bool) -> Result<Vec<(String, voxels::BuildSpec)>> {
    let key = if reuse { String::new() } else { std::env::var("OPENROUTER_API_KEY").context("set OPENROUTER_API_KEY")? };
    let jobs: Vec<_> = models.iter().map(|m| llm::display_name(m)).collect();
    let results: Vec<Result<(String, voxels::BuildSpec)>> = std::thread::scope(|s| {
        let handles: Vec<_> = jobs
            .iter()
            .map(|(id, name)| {
                let key = &key;
                s.spawn(move || -> Result<(String, voxels::BuildSpec)> {
                    let file = dir.join(format!("{}.json", slug(id)));
                    if reuse {
                        return Ok((name.clone(), load_spec(&file)?));
                    }
                    eprintln!("  asking {id} ...");
                    let started = std::time::Instant::now();
                    let g = llm::generate(key, id, prompt, grid)?;
                    std::fs::write(&file, serde_json::to_string_pretty(&g.spec)?)?;
                    std::fs::write(file.with_extension("raw.txt"), &g.raw)?;
                    eprintln!("  {id}: {} ops in {:.0}s", g.spec.ops.len(), started.elapsed().as_secs_f32());
                    Ok((name.clone(), g.spec))
                })
            })
            .collect();
        handles.into_iter().map(|h| h.join().expect("worker panicked")).collect()
    });
    let mut ok = Vec::new();
    for r in results {
        match r {
            Ok(v) => ok.push(v),
            Err(e) => eprintln!("  skipping: {e:#}"),
        }
    }
    if ok.len() < 2 {
        bail!("need at least 2 usable builds for a battle, got {}", ok.len());
    }
    Ok(ok)
}

fn slug(s: &str) -> String {
    let mut out = String::new();
    for c in s.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() || c == '.' {
            out.push(c);
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(60).collect()
}

fn splitmix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9e37_79b9_7f4a_7c15);
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    x ^ (x >> 31)
}

fn export_all(builds: &Path, out: &Path) -> Result<()> {
    let mut count = 0;
    let mut battles: Vec<_> = std::fs::read_dir(builds)?.filter_map(|e| e.ok()).filter(|e| e.path().is_dir()).collect();
    battles.sort_by_key(|e| e.file_name());
    for battle in battles {
        let slug = battle.file_name().to_string_lossy().to_string();
        let mut specs: Vec<_> = std::fs::read_dir(battle.path())?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        specs.sort();
        let dir = out.join(&slug);
        std::fs::create_dir_all(&dir)?;
        for path in specs {
            let n = path.file_stem().unwrap_or_default().to_string_lossy().to_string();
            let spec = load_spec(&path)?;
            let blocks = export::Blocks::from_grid(&spec.voxelize()?);
            let title = if spec.title.is_empty() { slug.clone() } else { spec.title.clone() };
            let world = format!("Bananini - {title}");
            let id = format!("{slug}/{n}");
            // Descriptive names so a kid's Downloads folder says what each file is: rocket_2.mcworld
            let file = format!("{}_{n}", slug.replace('-', "_"));
            std::fs::write(dir.join(format!("{file}_java.zip")), export::java_world_zip(&blocks, &world, &title)?)?;
            std::fs::write(dir.join(format!("{file}.mcworld")), export::bedrock_mcworld(&blocks, &world, &title, &id)?)?;
            eprintln!("  {id}: {} blocks, {:?}", blocks.blocks.len(), blocks.size.to_array());
            count += 1;
        }
    }
    eprintln!("  exported {count} builds -> {}", out.display());
    Ok(())
}

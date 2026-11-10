# voxreel

Renders satisfying voxel build videos as vertical shorts (1080x1920, 60 fps, H.264 + AAC),
including **build battles**: several LLMs get the same prompt, each designs a voxel build,
and the short shows them assembling block by block, then asks viewers to vote.

Original look (flat-coloured voxels, soft shadows, AO): no Minecraft assets involved.

## Requirements

- Rust (pinned in `mise.toml`), `ffmpeg` with libx264 on `PATH`
- `OPENROUTER_API_KEY` for battles
- a bold font: Impact / Arial Black are picked up on macOS automatically, else `--font file.ttf`

## Usage

```sh
# AI battle: generates builds (≈1 min), renders the short (≈1 min on an M1)
cargo run --release -- battle "a capybara relaxing in a hot spring"
# -> out/a-capybara-relaxing-in-a-hot-spring/battle.mp4 (+ each model's spec JSON and raw reply)

# re-render without calling the models again (tweak theme/timing for free)
cargo run --release -- battle "a capybara relaxing in a hot spring" --reuse --theme sunset

# pick the contenders; "id=Name" renames the label
cargo run --release -- battle "a haunted castle" --models "anthropic/claude-opus-5.5,x-ai/grok-4.7=GROK"

# render any build JSON (several files = battle layout)
cargo run --release -- render examples/lighthouse.json -o out/lighthouse.mp4

# preview a single frame instead of the whole video
cargo run --release -- battle "..." --reuse --still 12.5 --height 960
```

Useful flags: `--theme day|sunset|night|mint`, `--build-secs`, `--hold-secs`, `--lineup-secs`,
`--teaser-secs` (0 disables the opening hook), `--reveal-names`,
`--grid` (max build size the models may use), `--fps`, `--height`.

## Short structure

1. **Teaser** (1.5 s): the three finished builds, anonymous. The payoff up front is the hook.
2. **Builds** (~8 s each): `#1..n`, bottom-up "3D printer" assembly with dropping blocks,
   a block counter, rising pentatonic ticks, and a chime when the build is done.
3. **Lineup** (5 s): all builds side by side, "WHICH ONE IS THE BEST? 1, 2 OR 3?".

By default the video never mentions AI or model names (kid-friendly). `--reveal-names` shows
which model made which build in the lineup.

Model order is shuffled deterministically per prompt so the slot doesn't give the model away.

## Build DSL

Builds are JSON: `{"title", "size": [x, y, z], "ops": [...]}` with ops `box`, `sphere`,
`ellipsoid`, `cylinder`, `cone`, `line`, `voxels`, `erase` (Y up, ground at y = 0).
`cargo run --release -- prompt` prints the exact instructions the models get.
See `examples/lighthouse.json` for a hand-written one.

## Code map

- `voxels.rs`: DSL parsing (lenient with LLM output) and voxelization
- `anim.rs`: build order/timing, per-frame instances, per-corner AO
- `render.rs`, `shader.wgsl`: offscreen wgpu renderer (MSAA, shadow map, fog, sky)
- `short.rs`: shot list, camera fitting, captions, lineup panels, audio timeline
- `text.rs`, `audio.rs`, `video.rs`: captions, procedural sound, ffmpeg pipe
- `llm.rs`: OpenRouter client and the system prompt

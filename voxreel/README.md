# voxreel

Renders satisfying voxel build videos as vertical shorts (1080x1920, 60 fps, H.264 + AAC),
including **build battles**: several LLMs get the same prompt, each designs a voxel build,
and the short shows them assembling block by block, then asks viewers to vote.

Original look (flat-coloured voxels, soft shadows, AO): no Minecraft assets involved.
Published as **Bananini Games** (logo: `assets/logo.svg`, site: `web/`, see `web/README.md`).

## Requirements

- Rust (pinned in `mise.toml`), `ffmpeg` with libx264 on `PATH`
- `OPENROUTER_API_KEY` for the narrator (and for AI battles)
- captions use the embedded Lilita One font (`assets/fonts`, SIL OFL); `--font file.ttf` overrides

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

Hand-made battles live in `builds/<slug>/{1,2,3}.json`:

```sh
cargo run --release -- render builds/pirate-ship/*.json --title "PIRATE SHIP" --theme day -o out/pirate-ship/short.mp4
```

Useful flags: `--theme day|candy|sunset|night|moon|mint`, `--build-secs`, `--hold-secs`,
`--lineup-secs`, `--teaser-secs` (0 disables the opening hook), `--outro-secs` (0 disables the
closing card), `--site` (default `bananini.games`), `--volume` (default 0.8), `--reveal-names`,
`--grid` (max build size the models may use), `--fps`, `--height`.

Minecraft downloads (Java world zip + Bedrock `.mcworld`) for every build: `mise run export`;
`mise run deploy` exports, builds the website and deploys it to Cloudflare.

## Short structure

1. **Teaser** (1.5 s): the three finished builds, anonymous. The payoff up front is the hook.
2. **Builds** (~8 s each): `#1..n`, bottom-up "3D printer" assembly with dropping blocks,
   a block counter, rising pentatonic ticks, and a chime when the build is done.
3. **Lineup** (5 s): all builds side by side, "WHICH ONE IS THE BEST? 1, 2 OR 3?".
4. **Outro** (5.5 s): the Chimpanzini Bananini logo pops in over the sky, "GET THESE BUILDS FOR
   MINECRAFT! VISIT BANANINI.GAMES", and the narrator says the call to action.

Sound: the first second is silent (no ping on the hook), block ticks rise in pitch as the build
grows, transitions are soft whooshes, and the whole mix is at 0.8 of full scale.

## Narrator voice

All videos use the same narrator as the "Let it Plow" Roblox short, chosen by ear from several
ElevenLabs voices:

- **model `elevenlabs/eleven-v4`, voice `will`** (young, casual), via OpenRouter's
  `POST https://openrouter.ai/api/v1/audio/speech` with `{model, voice, input, response_format: "mp3"}`
- emotion tags in the text work and are used: `[excited]`, `[shouts]`, `[cheerfully]`, `[energetic]`
- constants: `TTS_MODEL` / `TTS_VOICE` in `src/audio.rs`; clips are cached in `$TMPDIR/voxreel-speech`
  and silence-trimmed before mixing

The macOS `say` voices were tried and rejected (too robotic). The logo on screen is the
Chimpanzini Bananini moment; the name is not spoken.

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
- `text.rs`, `audio.rs`, `video.rs`: captions + SVG logo sprites, procedural sound + narrator, ffmpeg pipe
- `llm.rs`: OpenRouter client and the system prompt
- `export.rs`: Minecraft Java/Bedrock downloads (NBT writer, colour-to-block mapping)

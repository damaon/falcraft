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

## Trend research

Channel goals and rules (kid-friendly, no brainrot, trending sounds added at upload, never baked
into the mp4) are in [STRATEGY.md](STRATEGY.md).

```sh
mise run trends                     # this week's top kid-friendly Minecraft/voxel Shorts
mise run trends -- --period day     # only today's uploads
```

`research/trends.py` (uv, yt-dlp + Shazam) reads the Shorts on the hashtag pages of each format
group in `research/config.toml`, drops blocklisted (brainrot / gross / scary) videos, ranks recent
uploads by views per day, finds channel outliers and identifies the sounds of the top videos.
Report: `research/out/<date>/report.md` (+ `trends.json`).

Video details (upload date, likes) come from the YouTube Data API when `YOUTUBE_API_KEY` is set
(free key, Google Cloud console: enable "YouTube Data API v3"). Without it the script opens watch
pages slowly with yt-dlp; YouTube answers bursts with a "not a bot" sign-in wall for a few hours,
and then the report falls back to titles only (oEmbed) without views per day or sounds.
`--cookies-from-browser chrome` avoids the wall but uses your own YouTube login. TikTok isn't
covered (no public listing API).

## "How it's made" series

Second format (from the trend research: process animations do well with kids): ~20 s, loops.

```sh
uv run --no-project tools/made.py              # (re)generate made/<slug>/{1..4,product}.json + recipe.json
cargo run --release -- made made/pizza/recipe.json        # -> out/made/pizza.mp4
cargo run --release -- made made/pizza/recipe.json --height 960 --still 7.8   # preview frame
```

1. **Hook** (1.2 s): the finished product orbiting, "HOW IS PIZZA MADE?".
2. **Steps** (2.6 s build + 0.8 s hold each): one diorama per step, "STEP n" + caption.
3. **Product** (3.2 s + 2.2 s): "TA-DA!"; its orbit continues into the hook, so the short loops.

No outro and no narrator by default (`--outro-secs 5.5` brings the logo card back): a trending
sound is added at upload in the YouTube app (see STRATEGY.md). A recipe (`made/<slug>/recipe.json`)
sets title, hook, theme, steps (`build`, `caption`, optional `label`) and the product; timings
(`step_secs`, `step_hold`, `product_secs`, `product_hold`, `hook_secs`) are optional. Builds are
written by `tools/made.py` on top of `tools/vox.py` (small voxel canvas: shapes, paint, rotation).

### Characters

Steps can have `actors` (`src/actor.rs`): blocky people with Minecraft player proportions
(32 px tall, 1 px = 0.6 voxel, so a 10-voxel table reaches their chest) who walk a path while the
scene assembles, then act:

```json
{"skin": "steve", "outfit": "chef", "hat": "chef", "path": [[28, 29], [10, 27]],
 "action": "stir", "item": "spoon", "face": [10, 18], "start": 0.0, "y": 1}
```

- `skin`: steve | alex; `outfit`: chef | farmer | beekeeper; `hat`: chef | straw | bee
- `action`: idle | stir | knead | chop | water | pick | carry | cheer | wave | eat
- `item` (right hand): spoon, tomato, strawberry, bucket, watering_can, pizza, rolling_pin, scoop,
  ice_cream, honey_jar, flower
- `path`/`face` are `[x, z]` in the build's voxels; `y` is the floor height (1 on a 1-voxel base,
  0 on the plate around the build); `start` delays the walk; `speed` is voxels per second.

Limbs are rotated boxes (instance quaternion in `render.rs`/`shader.wgsl`); footsteps are added to
the soundtrack. With characters the camera frames the whole scene from the first frame.

### Scenery and sound

`"backdrop": "backdrop.json"` puts every step in the middle of a static world (240x240 voxels,
generated by `tools/scenery.py`: `village`, `farm`, `meadow`). Its instances are computed once;
only the step's own build assembles. Rule of thumb for scenery: the camera orbits at ~55-80
voxels, so up to ~92 voxels from the centre only low things (paths, flowers, fences, fields), tall
things (houses, trees, barn, tower, windmill) from ~98 on, where they show up behind the scene.
With a backdrop the fog reaches further and shadows cover the area around the scene.

`"ambience": ["birds", "wind", "bees"]` adds procedural sound beds. "How it's made" shorts also
get their own foley (all synthesized in `audio.rs`): woody block "thocks" rising in pitch, a
caption "bloop", a question jingle on the hook, action sounds in sync with the arms (stir swish,
knead thump, chop knock, pick pop, pour, munch, cheer hops) and a "ta-da" fanfare on the product.
Battles keep their original marimba ticks.

## Story shorts (scripted, one world)

The next step of "how it's made": one village at Minecraft scale (1 block = 9.6 voxels, a
character is 2 blocks tall), characters following a script, a camera operator.

```sh
uv run --no-project tools/story.py pizza                 # writes story/pizza/{script,world}.json + props/
cargo run --release -- story story/pizza/script.json     # -> out/story/pizza.mp4 (~22 s to render)
cargo run --release -- story story/pizza/script.json --still 12 --height 960
```

- `tools/story.py` is a small DSL: `alex.walk(...)`, `alex.act("pick", 0.8, item="tomato")`,
  `basket.hand(t, alex)`, `pizza.build(t, x, y, z)`, `s.cam.shot(t0, t1, target, dist, ...)`,
  `s.step(t0, t1, "Step 1", "Pick the tomatoes")`. It computes all times and writes explicit
  keyframes; `src/story.rs` only plays them back.
- The world (`world.json`) is in blocks: each block is one big cube with a 16x16 pixel noise
  texture in the shader. Props, characters and food stay in voxels.
- `s.solid(...)` marks obstacles: `walk()` routes around them (shortest path over their corners),
  `s.check()` prints moments where characters overlap or stand inside something.
- The camera operator samples every shot at 0.1 s, swings sideways when a character or a block is
  in the way or something big is right in front of the lens, and smooths the swing.
- The hook shows the finished scene (`hook_scene`) under the question, then the story starts; the
  end returns to the same scene, so the short loops.

## Publishing

`publish/upload.py` uploads a finished short to the channel through the YouTube Data API
(`mise run upload -- <video.mp4> <publish/videos/name.json>`). Defaults: **private**, "made for kids"
(COPPA; this also turns comments off), Minecraft disclaimer + bananini.games link appended to the
description, a confirmation prompt before sending (`--yes` skips it), `--publish-at` schedules,
`--dry-run` only prints the request. Each upload costs 1,600 of the 10,000 daily quota units.

Uploads from an unverified Google Cloud project are forced to private by YouTube until the project
passes the YouTube API audit, so publish (or schedule) them in YouTube Studio. Trending sounds
can't be added through the API; for those, upload from the YouTube app instead.

One-time setup (same Google Cloud project as the `YOUTUBE_API_KEY`):
1. Google Auth Platform → Branding: app name, support email; Audience: External; add the channel
   owner's Google account as a test user (or publish the app: then the login shows an "unverified
   app" warning, but tokens don't expire after 7 days).
2. Clients → Create client → Desktop app → download the JSON to `publish/client_secret.json`.
3. `mise run upload -- --login`: log in, pick the channel (Brand Account), allow. The token is
   cached in `publish/.token.json`. Both files are git-ignored.

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

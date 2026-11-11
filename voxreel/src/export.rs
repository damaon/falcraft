//! Minecraft downloads for a build, no mods needed:
//! - Java: a world folder (zip) with a flat creative world and a datapack that places the build
//!   as a structure next to spawn on first load.
//! - Bedrock: a `.mcworld` with a flat creative world and a behavior pack that loads the build
//!   as a `.mcstructure` a few seconds after the world opens.
//!
//! Neither world ships terrain data: the game generates the flat terrain itself from level.dat,
//! which avoids writing Anvil / LevelDB chunk formats by hand.

use crate::voxels::VoxelGrid;
use anyhow::Result;
use flate2::{write::GzEncoder, Compression};
use glam::IVec3;
use std::io::{Cursor, Write};
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

// ---------------------------------------------------------------- NBT

#[derive(Clone)]
pub enum Tag {
    Byte(i8),
    Int(i32),
    Long(i64),
    Float(f32),
    String(String),
    /// Element type id + items (all of that type).
    List(u8, Vec<Tag>),
    Compound(Vec<(String, Tag)>),
}

impl Tag {
    fn id(&self) -> u8 {
        match self {
            Tag::Byte(_) => 1,
            Tag::Int(_) => 3,
            Tag::Long(_) => 4,
            Tag::Float(_) => 5,
            Tag::String(_) => 8,
            Tag::List(..) => 9,
            Tag::Compound(_) => 10,
        }
    }
}

fn s(v: &str) -> Tag {
    Tag::String(v.to_string())
}
fn compound(items: Vec<(&str, Tag)>) -> Tag {
    Tag::Compound(items.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}
fn ints(v: &[i32]) -> Tag {
    Tag::List(3, v.iter().map(|i| Tag::Int(*i)).collect())
}
fn list_of(id: u8, v: Vec<Tag>) -> Tag {
    Tag::List(if v.is_empty() { 0 } else { id }, v)
}

/// Java is big-endian, Bedrock little-endian; otherwise the format is identical.
struct NbtWriter {
    out: Vec<u8>,
    le: bool,
}

impl NbtWriter {
    fn u16(&mut self, v: u16) {
        self.out.extend(if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }
    fn i32(&mut self, v: i32) {
        self.out.extend(if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }
    fn i64(&mut self, v: i64) {
        self.out.extend(if self.le { v.to_le_bytes() } else { v.to_be_bytes() });
    }
    fn str(&mut self, v: &str) {
        self.u16(v.len() as u16);
        self.out.extend(v.as_bytes());
    }
    fn payload(&mut self, t: &Tag) {
        match t {
            Tag::Byte(b) => self.out.push(*b as u8),
            Tag::Int(i) => self.i32(*i),
            Tag::Long(l) => self.i64(*l),
            Tag::Float(f) => self.i32(f.to_bits() as i32),
            Tag::String(v) => self.str(v),
            Tag::List(id, items) => {
                self.out.push(*id);
                self.i32(items.len() as i32);
                for it in items {
                    self.payload(it);
                }
            }
            Tag::Compound(items) => {
                for (k, v) in items {
                    self.out.push(v.id());
                    self.str(k);
                    self.payload(v);
                }
                self.out.push(0);
            }
        }
    }
}

pub fn nbt_bytes(root: &Tag, little_endian: bool) -> Vec<u8> {
    let mut w = NbtWriter { out: Vec::new(), le: little_endian };
    w.out.push(root.id());
    w.str("");
    w.payload(root);
    w.out
}

fn gzip(data: &[u8]) -> Result<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(data)?;
    Ok(e.finish()?)
}

// ---------------------------------------------------------------- palette

/// Blocks with identical ids in Java and modern Bedrock, so both downloads look the same.
/// Colours are the blocks' average texture colours (same values as the falcraft mod's BlockMapper).
const PALETTE: &[(&str, u32)] = &[
    ("white_concrete", 0xF9FFFE), ("light_gray_concrete", 0x7D7D73), ("gray_concrete", 0x36393D),
    ("black_concrete", 0x080A0F), ("brown_concrete", 0x5C3A24), ("red_concrete", 0x8E2121),
    ("orange_concrete", 0xE06101), ("yellow_concrete", 0xF1AF15), ("lime_concrete", 0x5EA818),
    ("green_concrete", 0x495B24), ("cyan_concrete", 0x157788), ("light_blue_concrete", 0x2389C6),
    ("blue_concrete", 0x2C2E8F), ("purple_concrete", 0x64209C), ("magenta_concrete", 0xA9309F),
    ("pink_concrete", 0xD5658F),
    ("white_wool", 0xE9ECEC), ("light_gray_wool", 0x8E8E86), ("gray_wool", 0x3E4447),
    ("black_wool", 0x141519), ("brown_wool", 0x724728), ("red_wool", 0xA12722),
    ("orange_wool", 0xF07613), ("yellow_wool", 0xF8C627), ("lime_wool", 0x70B919),
    ("green_wool", 0x546D1B), ("cyan_wool", 0x158991), ("light_blue_wool", 0x3AAFD9),
    ("blue_wool", 0x353A9E), ("purple_wool", 0x7B2BAD), ("magenta_wool", 0xBD44B3),
    ("pink_wool", 0xED8DAC),
    ("white_terracotta", 0xD1B1A1), ("light_gray_terracotta", 0x876B62), ("gray_terracotta", 0x392A23),
    ("black_terracotta", 0x251610), ("brown_terracotta", 0x4D3224), ("red_terracotta", 0x8F3D2E),
    ("orange_terracotta", 0xA05325), ("yellow_terracotta", 0xBA8523), ("lime_terracotta", 0x677534),
    ("green_terracotta", 0x4C532A), ("cyan_terracotta", 0x575B5B), ("light_blue_terracotta", 0x706C89),
    ("blue_terracotta", 0x4A3B5B), ("purple_terracotta", 0x764556), ("magenta_terracotta", 0x95576C),
    ("pink_terracotta", 0xA14E4E),
];

fn lab(rgb: [u8; 3]) -> [f32; 3] {
    let lin = |c: u8| {
        let c = c as f32 / 255.0;
        if c <= 0.04045 { c / 12.92 } else { ((c + 0.055) / 1.055).powf(2.4) }
    };
    let (r, g, b) = (lin(rgb[0]), lin(rgb[1]), lin(rgb[2]));
    let x = (0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047;
    let y = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    let z = (0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883;
    let f = |t: f32| if t > 0.008856 { t.cbrt() } else { 7.787 * t + 16.0 / 116.0 };
    let (fx, fy, fz) = (f(x), f(y), f(z));
    [116.0 * fy - 16.0, 500.0 * (fx - fy), 200.0 * (fy - fz)]
}

/// Index into PALETTE of the perceptually closest block.
fn nearest(rgb: [u8; 3]) -> usize {
    let target = lab(rgb);
    let mut best = (f32::MAX, 0);
    for (i, (_, hex)) in PALETTE.iter().enumerate() {
        let l = lab([(hex >> 16) as u8, (hex >> 8) as u8, *hex as u8]);
        let d = (0..3).map(|k| (l[k] - target[k]).powi(2)).sum::<f32>();
        if d < best.0 {
            best = (d, i);
        }
    }
    best.1
}

/// The build re-based to its tight bounds, with block palette indices.
pub struct Blocks {
    pub size: IVec3,
    /// (position, PALETTE index)
    pub blocks: Vec<(IVec3, usize)>,
}

impl Blocks {
    pub fn from_grid(grid: &VoxelGrid) -> Self {
        let (lo, hi) = grid.bounds().unwrap_or((IVec3::ZERO, IVec3::ONE));
        let blocks = grid.iter().map(|(p, c)| (p - lo, nearest(c))).collect();
        Self { size: hi - lo, blocks }
    }
}

// ---------------------------------------------------------------- Java

/// 1.21.1. Newer game versions upgrade older data automatically.
const JAVA_DATA_VERSION: i32 = 3955;
const JAVA_PACK_FORMAT: i32 = 48;
/// Flat world: bedrock + 2 dirt + grass from y=-64, so the surface is y=-61 and you stand on -60.
const GROUND_Y: i32 = -60;
/// Gap between spawn and the front of the build.
const SPAWN_GAP: i32 = 6;

fn java_structure(b: &Blocks) -> Result<Vec<u8>> {
    let mut used: Vec<usize> = b.blocks.iter().map(|(_, i)| *i).collect();
    used.sort_unstable();
    used.dedup();
    let state_of = |i: usize| used.binary_search(&i).unwrap() as i32;
    let palette = used.iter().map(|i| compound(vec![("Name", s(&format!("minecraft:{}", PALETTE[*i].0)))])).collect();
    let blocks = b
        .blocks
        .iter()
        .map(|(p, i)| compound(vec![("pos", ints(&[p.x, p.y, p.z])), ("state", Tag::Int(state_of(*i)))]))
        .collect();
    let root = compound(vec![
        ("DataVersion", Tag::Int(JAVA_DATA_VERSION)),
        ("size", ints(&[b.size.x, b.size.y, b.size.z])),
        ("palette", list_of(10, palette)),
        ("blocks", list_of(10, blocks)),
        ("entities", list_of(10, vec![])),
    ]);
    gzip(&nbt_bytes(&root, false))
}

fn java_level_dat(name: &str) -> Result<Vec<u8>> {
    let layer = |block: &str, height: i32| compound(vec![("block", s(block)), ("height", Tag::Int(height))]);
    let overworld = compound(vec![
        ("type", s("minecraft:overworld")),
        (
            "generator",
            compound(vec![
                ("type", s("minecraft:flat")),
                (
                    "settings",
                    compound(vec![
                        ("biome", s("minecraft:plains")),
                        ("features", Tag::Byte(0)),
                        ("lakes", Tag::Byte(0)),
                        ("structure_overrides", list_of(8, vec![])),
                        (
                            "layers",
                            list_of(10, vec![layer("minecraft:bedrock", 1), layer("minecraft:dirt", 2), layer("minecraft:grass_block", 1)]),
                        ),
                    ]),
                ),
            ]),
        ),
    ]);
    let nether = compound(vec![
        ("type", s("minecraft:the_nether")),
        (
            "generator",
            compound(vec![
                ("type", s("minecraft:noise")),
                ("settings", s("minecraft:nether")),
                ("biome_source", compound(vec![("type", s("minecraft:multi_noise")), ("preset", s("minecraft:nether"))])),
            ]),
        ),
    ]);
    let end = compound(vec![
        ("type", s("minecraft:the_end")),
        (
            "generator",
            compound(vec![
                ("type", s("minecraft:noise")),
                ("settings", s("minecraft:end")),
                ("biome_source", compound(vec![("type", s("minecraft:the_end"))])),
            ]),
        ),
    ]);
    let rule = |v: &str| s(v);
    let data = compound(vec![
        ("DataVersion", Tag::Int(JAVA_DATA_VERSION)),
        (
            "version",
            compound(vec![
                ("Id", Tag::Int(JAVA_DATA_VERSION)),
                ("Name", s("1.21.1")),
                ("Series", s("main")),
                ("Snapshot", Tag::Byte(0)),
            ]),
        ),
        ("LevelName", s(name)),
        ("GameType", Tag::Int(1)),
        ("allowCommands", Tag::Byte(1)),
        ("hardcore", Tag::Byte(0)),
        ("Difficulty", Tag::Byte(0)),
        ("initialized", Tag::Byte(1)),
        ("SpawnX", Tag::Int(0)),
        ("SpawnY", Tag::Int(GROUND_Y)),
        ("SpawnZ", Tag::Int(0)),
        ("SpawnAngle", Tag::Float(0.0)),
        ("DayTime", Tag::Long(6000)),
        ("Time", Tag::Long(0)),
        ("raining", Tag::Byte(0)),
        ("thundering", Tag::Byte(0)),
        (
            "GameRules",
            compound(vec![
                ("doDaylightCycle", rule("false")),
                ("doWeatherCycle", rule("false")),
                ("doMobSpawning", rule("false")),
                ("doInsomnia", rule("false")),
                ("spawnRadius", rule("0")),
            ]),
        ),
        (
            "DataPacks",
            compound(vec![("Enabled", list_of(8, vec![s("vanilla"), s("file/bananini")])), ("Disabled", list_of(8, vec![]))]),
        ),
        (
            "WorldGenSettings",
            compound(vec![
                ("seed", Tag::Long(0)),
                ("generate_features", Tag::Byte(0)),
                ("bonus_chest", Tag::Byte(0)),
                (
                    "dimensions",
                    compound(vec![("minecraft:overworld", overworld), ("minecraft:the_nether", nether), ("minecraft:the_end", end)]),
                ),
            ]),
        ),
    ]);
    gzip(&nbt_bytes(&compound(vec![("Data", data)]), false))
}

fn origin(b: &Blocks) -> IVec3 {
    IVec3::new(-b.size.x / 2, GROUND_Y, SPAWN_GAP)
}

pub fn java_world_zip(b: &Blocks, world_name: &str, title: &str) -> Result<Vec<u8>> {
    let o = origin(b);
    let far = o + b.size;
    let dp = format!("{world_name}/datapacks/bananini");
    let mcmeta = serde_json::json!({
        "pack": {
            "pack_format": JAVA_PACK_FORMAT,
            "supported_formats": { "min_inclusive": JAVA_PACK_FORMAT, "max_inclusive": 9999 },
            "min_format": JAVA_PACK_FORMAT,
            "max_format": 9999,
            "description": format!("Bananini Games: {title}")
        }
    });
    // The structure is placed once, after its chunks are force-loaded.
    let load = format!(
        "scoreboard objectives add bananini dummy\nforceload add {} {} {} {}\nschedule function bananini:place 2s\n",
        o.x - 2,
        o.z - 2,
        far.x + 2,
        far.z + 2
    );
    let place = format!(
        "execute unless score #placed bananini matches 1 run place template bananini:build {} {} {}\nscoreboard players set #placed bananini 1\nforceload remove all\n",
        o.x, o.y, o.z
    );
    let files: Vec<(String, Vec<u8>)> = vec![
        (format!("{world_name}/level.dat"), java_level_dat(world_name)?),
        (format!("{dp}/pack.mcmeta"), serde_json::to_vec_pretty(&mcmeta)?),
        (format!("{dp}/data/bananini/structure/build.nbt"), java_structure(b)?),
        (format!("{dp}/data/bananini/function/load.mcfunction"), load.into_bytes()),
        (format!("{dp}/data/bananini/function/place.mcfunction"), place.into_bytes()),
        (format!("{dp}/data/minecraft/tags/function/load.json"), br#"{"values":["bananini:load"]}"#.to_vec()),
    ];
    zip_files(&files)
}

// ---------------------------------------------------------------- Bedrock

/// 1.21.40, encoded as major<<24 | minor<<16 | patch<<8 | revision.
const BEDROCK_BLOCK_VERSION: i32 = (1 << 24) | (21 << 16) | (40 << 8);

fn bedrock_structure(b: &Blocks) -> Vec<u8> {
    let mut used: Vec<usize> = b.blocks.iter().map(|(_, i)| *i).collect();
    used.sort_unstable();
    used.dedup();
    let (w, h, d) = (b.size.x, b.size.y, b.size.z);
    // ZYX order: z changes fastest, then y, then x.
    let mut layer0 = vec![-1i32; (w * h * d) as usize];
    for (p, i) in &b.blocks {
        layer0[((p.x * h + p.y) * d + p.z) as usize] = used.binary_search(i).unwrap() as i32;
    }
    let layer1 = vec![-1i32; layer0.len()];
    let palette = used
        .iter()
        .map(|i| {
            compound(vec![
                ("name", s(&format!("minecraft:{}", PALETTE[*i].0))),
                ("states", compound(vec![])),
                ("version", Tag::Int(BEDROCK_BLOCK_VERSION)),
            ])
        })
        .collect();
    let root = compound(vec![
        ("format_version", Tag::Int(1)),
        ("size", ints(&[w, h, d])),
        ("structure_world_origin", ints(&[0, 0, 0])),
        (
            "structure",
            compound(vec![
                ("block_indices", Tag::List(9, vec![ints(&layer0), ints(&layer1)])),
                ("entities", list_of(10, vec![])),
                (
                    "palette",
                    compound(vec![(
                        "default",
                        compound(vec![("block_palette", list_of(10, palette)), ("block_position_data", compound(vec![]))]),
                    )]),
                ),
            ]),
        ),
    ]);
    nbt_bytes(&root, true)
}

fn bedrock_level_dat(name: &str) -> Vec<u8> {
    let layers = serde_json::json!({
        "biome_id": 1,
        "block_layers": [
            { "block_name": "minecraft:bedrock", "count": 1 },
            { "block_name": "minecraft:dirt", "count": 2 },
            { "block_name": "minecraft:grass_block", "count": 1 }
        ],
        "encoding_version": 6,
        "structure_options": null,
        "world_type": "Overworld"
    });
    let ver = |v: &[i32]| ints(v);
    let root = compound(vec![
        ("LevelName", s(name)),
        ("GameType", Tag::Int(1)),
        ("Generator", Tag::Int(2)),
        ("FlatWorldLayers", s(&layers.to_string())),
        ("RandomSeed", Tag::Long(0)),
        ("SpawnX", Tag::Int(0)),
        ("SpawnY", Tag::Int(GROUND_Y)),
        ("SpawnZ", Tag::Int(0)),
        ("StorageVersion", Tag::Int(10)),
        ("lastOpenedWithVersion", ver(&[1, 21, 40, 0, 0])),
        ("MinimumCompatibleClientVersion", ver(&[1, 21, 0, 0, 0])),
        ("Difficulty", Tag::Int(0)),
        ("commandsEnabled", Tag::Byte(1)),
        ("hasBeenLoadedInCreative", Tag::Byte(1)),
        ("doDaylightCycle", Tag::Byte(0)),
        ("doweathercycle", Tag::Byte(0)),
        ("domobspawning", Tag::Byte(0)),
        ("showcoordinates", Tag::Byte(1)),
        ("spawnradius", Tag::Int(0)),
        ("Time", Tag::Long(6000)),
        ("experiments", compound(vec![])),
    ]);
    let body = nbt_bytes(&root, true);
    // Bedrock level.dat starts with an 8-byte header: storage version + payload length.
    let mut out = Vec::with_capacity(body.len() + 8);
    out.extend(10i32.to_le_bytes());
    out.extend((body.len() as i32).to_le_bytes());
    out.extend(body);
    out
}

/// Deterministic UUID-shaped id so re-exports keep the same pack identity.
fn pseudo_uuid(seed: &str) -> String {
    let mut h: u128 = 0x6c62272e07bb014262b821756295c58d;
    for b in seed.bytes() {
        h ^= b as u128;
        h = h.wrapping_mul(0x0000000001000000000000000000013B);
    }
    let x = format!("{h:032x}");
    format!("{}-{}-4{}-a{}-{}", &x[0..8], &x[8..12], &x[13..16], &x[17..20], &x[20..32])
}

pub fn bedrock_mcworld(b: &Blocks, world_name: &str, title: &str, id: &str) -> Result<Vec<u8>> {
    let o = origin(b);
    let (pack_uuid, module_uuid) = (pseudo_uuid(&format!("{id}:pack")), pseudo_uuid(&format!("{id}:module")));
    let manifest = serde_json::json!({
        "format_version": 2,
        "header": {
            "name": format!("Bananini Games: {title}"),
            "description": "Places the build next to spawn when the world opens.",
            "uuid": pack_uuid,
            "version": [1, 0, 0],
            "min_engine_version": [1, 21, 0]
        },
        "modules": [{ "type": "data", "uuid": module_uuid, "version": [1, 0, 0] }]
    });
    let world_packs = serde_json::json!([{ "pack_id": pack_uuid, "version": [1, 0, 0] }]);
    // Runs every tick; loads the structure once, ~3 s in, when the spawn area is surely loaded.
    let boot = format!(
        "scoreboard objectives add bananini dummy\nscoreboard players add tick bananini 1\nexecute if score tick bananini matches 60 run structure load bananini:build {} {} {}\n",
        o.x, o.y, o.z
    );
    let bp = "behavior_packs/bananini";
    let files: Vec<(String, Vec<u8>)> = vec![
        ("level.dat".into(), bedrock_level_dat(world_name)),
        ("levelname.txt".into(), world_name.as_bytes().to_vec()),
        ("world_behavior_packs.json".into(), serde_json::to_vec_pretty(&world_packs)?),
        (format!("{bp}/manifest.json"), serde_json::to_vec_pretty(&manifest)?),
        (format!("{bp}/structures/bananini/build.mcstructure"), bedrock_structure(b)),
        (format!("{bp}/functions/tick.json"), br#"{"values":["bananini/boot"]}"#.to_vec()),
        (format!("{bp}/functions/bananini/boot.mcfunction"), boot.into_bytes()),
    ];
    zip_files(&files)
}

fn zip_files(files: &[(String, Vec<u8>)]) -> Result<Vec<u8>> {
    let mut z = ZipWriter::new(Cursor::new(Vec::new()));
    let opts = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    for (name, data) in files {
        z.start_file(name.as_str(), opts)?;
        z.write_all(data)?;
    }
    Ok(z.finish()?.into_inner())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearest_picks_obvious_blocks() {
        assert_eq!(PALETTE[nearest([255, 255, 255])].0, "white_concrete");
        assert_eq!(PALETTE[nearest([10, 10, 14])].0, "black_concrete");
        assert!(PALETTE[nearest([230, 40, 40])].0.starts_with("red_"));
    }

    #[test]
    fn nbt_endianness_and_header() {
        let t = compound(vec![("a", Tag::Int(1))]);
        assert_eq!(nbt_bytes(&t, false), vec![10, 0, 0, 3, 0, 1, b'a', 0, 0, 0, 1, 0]);
        assert_eq!(nbt_bytes(&t, true), vec![10, 0, 0, 3, 1, 0, b'a', 1, 0, 0, 0, 0]);
    }
}

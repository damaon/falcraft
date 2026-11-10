//! Asks LLMs (via OpenRouter) to design a voxel build in the `voxels::BuildSpec` DSL.

use crate::voxels::BuildSpec;
use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::time::Duration;

const ENDPOINT: &str = "https://openrouter.ai/api/v1/chat/completions";

pub fn system_prompt(grid: i32) -> String {
    format!(
        r##"You are a world-class voxel artist. Build the subject the user asks for as voxel art, written in a JSON DSL.
Reply with ONLY one JSON object, no prose, no markdown:
{{"title": "<short name>", "size": [X, Y, Z], "ops": [ ... ]}}

Coordinate system: integer voxel coordinates, x = right, y = UP, z = towards the viewer.
Every size component must be <= {grid}. The ground is y = 0 and the build must stand on it.
Keep the build roughly centred in x and z.

Ops are applied in order; later ops overwrite earlier voxels:
- {{"op":"box","from":[x,y,z],"to":[x,y,z],"color":"#rrggbb","hollow":false}}   (inclusive corners)
- {{"op":"sphere","center":[x,y,z],"radius":r,"color":"#rrggbb","hollow":false}}
- {{"op":"ellipsoid","center":[x,y,z],"radii":[rx,ry,rz],"color":"#rrggbb","hollow":false}}
- {{"op":"cylinder","base":[x,y,z],"radius":r,"height":h,"axis":"y","color":"#rrggbb","hollow":false}}   (axis x|y|z, grows from base along +axis; negative height grows the other way)
- {{"op":"cone","base":[x,y,z],"radius":r,"height":h,"axis":"y","color":"#rrggbb"}}   (tip at base+height)
- {{"op":"line","from":[x,y,z],"to":[x,y,z],"radius":0.5,"color":"#rrggbb"}}   (thick line: legs, branches, horns)
- {{"op":"voxels","points":[[x,y,z],...],"color":"#rrggbb"}}   (single-voxel details: eyes, windows, buttons)
- {{"op":"erase","from":[x,y,z],"to":[x,y,z]}}   (carve: doors, mouths, arches)

What makes a great build:
- An instantly recognisable silhouette from every side; exaggerate the defining features.
- Use most of the grid: the subject should be big, not a tiny object in an empty box.
- 6-14 harmonious colours with shading: darker undersides, lighter tops, accent colours for details.
- Plenty of detail: 60-250 ops. Faces need eyes; buildings need windows, doors, roofs; nature needs variation.
- Optionally a small base/ground patch (grass, sand, rock) under the subject for a diorama look."##
    )
}

pub struct Generation {
    pub spec: BuildSpec,
    pub raw: String,
}

pub fn generate(api_key: &str, model: &str, prompt: &str, grid: i32) -> Result<Generation> {
    let agent = ureq::AgentBuilder::new().timeout(Duration::from_secs(900)).build();
    let mut messages = vec![
        json!({"role": "system", "content": system_prompt(grid)}),
        json!({"role": "user", "content": format!("Build: {prompt}")}),
    ];
    let mut last_err = None;
    // One repair round: models occasionally truncate or wrap the JSON.
    for attempt in 0..2 {
        let body = json!({
            "model": model,
            "messages": messages,
            "max_tokens": 32000,
            "temperature": 0.7,
            "usage": {"include": true},
        });
        let resp = agent
            .post(ENDPOINT)
            .set("Authorization", &format!("Bearer {api_key}"))
            .set("X-Title", "voxreel")
            .send_json(body);
        let resp: Value = match resp {
            Ok(r) => r.into_json()?,
            Err(ureq::Error::Status(code, r)) => {
                bail!("{model}: HTTP {code}: {}", r.into_string().unwrap_or_default())
            }
            Err(e) => return Err(e).context(format!("{model}: request failed")),
        };
        if let Some(cost) = resp["usage"]["cost"].as_f64() {
            eprintln!("  {model}: ${cost:.3} ({} output tokens)", resp["usage"]["completion_tokens"]);
        }
        let text = resp["choices"][0]["message"]["content"].as_str().unwrap_or_default().to_string();
        match BuildSpec::from_llm_text(&text).and_then(|s| {
            let n = s.voxelize()?.count();
            if n < 20 {
                bail!("build has only {n} voxels");
            }
            Ok(s)
        }) {
            Ok(spec) => return Ok(Generation { spec, raw: text }),
            Err(e) => {
                eprintln!("  {model}: attempt {} unusable: {e:#}", attempt + 1);
                messages.push(json!({"role": "assistant", "content": text}));
                messages.push(json!({"role": "user", "content": format!(
                    "That response could not be used ({e}). Reply again with ONLY the complete JSON object."
                )}));
                last_err = Some(e);
            }
        }
    }
    Err(last_err.unwrap()).context(format!("{model}: no usable build"))
}

/// "anthropic/claude-opus-5.5" -> "CLAUDE OPUS 5.5"; "id=Custom Name" overrides.
pub fn display_name(model: &str) -> (String, String) {
    if let Some((id, name)) = model.split_once('=') {
        return (id.to_string(), name.to_string());
    }
    let short = model.rsplit('/').next().unwrap_or(model).split(':').next().unwrap_or(model);
    (model.to_string(), short.replace('-', " ").to_uppercase())
}

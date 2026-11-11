struct U {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    sun: vec4<f32>,
    cam: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground: vec4<f32>,
    ground2: vec4<f32>,
    // time, clouds, stars, -
    extra: vec4<f32>,
    // width, height
    res: vec4<f32>,
    // fog_near, fog_far, shadow texel, ground y
    params: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var shadow_tex: texture_depth_2d;
@group(0) @binding(2) var shadow_smp: sampler_comparison;

struct VIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) corner: u32,
};

struct IIn {
    @location(3) pos: vec3<f32>,
    @location(4) color: vec4<f32>,
    @location(5) size: vec3<f32>,
    @location(6) glow: f32,
    @location(7) ao: vec2<u32>,
};

fn world_of(v: VIn, i: IIn) -> vec3<f32> {
    return i.pos + v.pos * i.size;
}

@vertex
fn vs_shadow(v: VIn, i: IIn) -> @builtin(position) vec4<f32> {
    return u.light_vp * vec4<f32>(world_of(v, i), 1.0);
}

struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec3<f32>,
    @location(3) ao: f32,
    @location(4) glow: f32,
};

@vertex
fn vs_cube(v: VIn, i: IIn) -> VOut {
    var o: VOut;
    let w = world_of(v, i);
    o.clip = u.view_proj * vec4<f32>(w, 1.0);
    o.world = w;
    o.normal = v.normal;
    o.color = pow(i.color.rgb, vec3<f32>(2.2));
    let word = select(i.ao.x, i.ao.y, v.corner >= 16u);
    let shift = (v.corner % 16u) * 2u;
    o.ao = f32((word >> shift) & 3u) / 3.0;
    o.glow = i.glow;
    return o;
}

fn shadow_at(world: vec3<f32>) -> f32 {
    let lp = u.light_vp * vec4<f32>(world, 1.0);
    let ndc = lp.xyz / lp.w;
    let uv = ndc.xy * vec2<f32>(0.5, -0.5) + vec2<f32>(0.5);
    if (uv.x < 0.0 || uv.y < 0.0 || uv.x > 1.0 || uv.y > 1.0 || ndc.z > 1.0) {
        return 1.0;
    }
    var sum = 0.0;
    let texel = u.params.z;
    for (var y = -1; y <= 1; y++) {
        for (var x = -1; x <= 1; x++) {
            let off = vec2<f32>(f32(x), f32(y)) * texel * 1.5;
            sum += textureSampleCompareLevel(shadow_tex, shadow_smp, uv + off, ndc.z - 0.0008);
        }
    }
    return sum / 9.0;
}

fn hash2(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u2 = f * f * (3.0 - 2.0 * f);
    return mix(mix(hash2(i), hash2(i + vec2<f32>(1.0, 0.0)), u2.x),
               mix(hash2(i + vec2<f32>(0.0, 1.0)), hash2(i + vec2<f32>(1.0, 1.0)), u2.x), u2.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var a = 0.5;
    var q = p;
    for (var i = 0; i < 5; i++) {
        v += a * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.0, 9.0);
        a *= 0.5;
    }
    return v;
}

// Darkens the corners a little so the eye stays on the build.
fn vignette(c: vec3<f32>, frag: vec2<f32>) -> vec3<f32> {
    let uv = frag / u.res.xy - vec2<f32>(0.5);
    let d = length(uv * vec2<f32>(1.0, 0.75));
    return c * (1.0 - 0.35 * smoothstep(0.35, 0.85, d));
}

fn sky_color(dir: vec3<f32>) -> vec3<f32> {
    // The camera only ever sees a few degrees above the horizon, so everything here is scaled
    // to that band: azimuth/elevation coordinates instead of a sky dome.
    // Elevation above the *virtual* horizon: where the ground disappears into fog. The fog
    // blends into this same function, so ground, haze and sky form one seamless backdrop.
    let h0 = -(u.cam.y - u.params.w) / u.params.y;
    let elev = dir.y - h0;
    let az = atan2(dir.x, dir.z);
    var c = mix(u.sky_horizon.rgb, u.sky_top.rgb, smoothstep(-0.01, 0.16, elev));
    // Warm glow around the sun's azimuth.
    let sd = max(dot(normalize(vec3<f32>(dir.x, 0.0, dir.z)), normalize(vec3<f32>(u.sun.x, 0.0, u.sun.z))), 0.0);
    c += vec3<f32>(1.0, 0.85, 0.6) * pow(sd, 8.0) * 0.18 * smoothstep(0.12, 0.0, elev) * (1.0 - u.extra.z);
    // Puffy clouds just above the horizon, drifting slowly.
    if (u.extra.y > 0.0) {
        let p = vec2<f32>(az * 3.0 + u.extra.x * 0.02, elev * 16.0);
        let n = fbm(p);
        let band = smoothstep(0.005, 0.03, elev);
        let cover = smoothstep(0.47, 0.62, n) * band * u.extra.y;
        let shade = mix(0.86, 1.0, smoothstep(0.0, 0.06, elev));
        c = mix(c, vec3<f32>(1.0, 1.0, 1.0) * shade, cover * 0.95);
    }
    // Stars for night themes.
    if (u.extra.z > 0.0) {
        let cell = floor(vec2<f32>(az * 450.0, elev * 450.0));
        let h = hash2(cell);
        let star = step(0.985, h) * smoothstep(0.005, 0.04, elev);
        c += vec3<f32>(star * u.extra.z * (0.5 + 0.5 * hash2(cell + 3.7)));
    }
    return c;
}

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    // Gentle shoulder so glow/bright faces roll off instead of clipping.
    return c * (1.0 + c / 6.0) / (1.0 + c);
}

fn fog(c: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let d = distance(world, u.cam.xyz);
    let f = smoothstep(u.params.x, u.params.y, d);
    return mix(c, sky_color(normalize(world - u.cam.xyz)), f);
}

fn light(albedo: vec3<f32>, n: vec3<f32>, world: vec3<f32>, ao: f32) -> vec3<f32> {
    let sun = u.sun.xyz;
    let ndl = max(dot(n, sun), 0.0);
    let sh = shadow_at(world + n * 0.04);
    let sky = mix(u.ground.rgb * 0.7, u.sky_top.rgb * 0.6 + vec3<f32>(0.25), n.y * 0.5 + 0.5);
    let ambient = sky * mix(0.25, 1.0, ao);
    let direct = vec3<f32>(1.0, 0.94, 0.84) * ndl * sh * mix(0.6, 1.0, ao);
    return albedo * (direct * 1.6 + ambient * 0.9);
}

@fragment
fn fs_cube(i: VOut) -> @location(0) vec4<f32> {
    let n = normalize(i.normal);
    var c = light(i.color, n, i.world, i.ao);
    c += i.color * i.glow * 2.0 + vec3<f32>(i.glow * 0.25);
    return vec4<f32>(vignette(fog(tonemap(c), i.world), i.clip.xy), 1.0);
}

struct GOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
};

@vertex
fn vs_ground(@builtin(vertex_index) vi: u32) -> GOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let c = corners[vi] * 800.0;
    let w = vec3<f32>(c.x, u.params.w, c.y);
    var o: GOut;
    o.clip = u.view_proj * vec4<f32>(w, 1.0);
    o.world = w;
    return o;
}

@fragment
fn fs_ground(i: GOut) -> @location(0) vec4<f32> {
    // Soft anti-aliased 4x4 checker plus a faint unit grid.
    let p = i.world.xz;
    let s = sin(p.x * 0.7853982) * sin(p.y * 0.7853982);
    let k = clamp(s / max(fwidth(s) * 1.5, 1e-4) * 0.5 + 0.5, 0.0, 1.0);
    let g = abs(fract(p - 0.5) - 0.5) / fwidth(p);
    let line = 1.0 - min(min(g.x, g.y), 1.0);
    let base = mix(u.ground.rgb, u.ground2.rgb, k) * (1.0 - line * 0.05);
    let c = light(base, vec3<f32>(0.0, 1.0, 0.0), i.world, 1.0);
    return vec4<f32>(vignette(fog(tonemap(c), i.world), i.clip.xy), 1.0);
}

struct SOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};

@vertex
fn vs_sky(@builtin(vertex_index) vi: u32) -> SOut {
    let xy = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
    var o: SOut;
    o.clip = vec4<f32>(xy * 2.0 - 1.0, 1.0, 1.0);
    o.ndc = xy * 2.0 - 1.0;
    return o;
}

@fragment
fn fs_sky(i: SOut) -> @location(0) vec4<f32> {
    // Gradient by view direction so the horizon matches the fogged ground exactly.
    let w = u.inv_view_proj * vec4<f32>(i.ndc, 1.0, 1.0);
    let dir = normalize(w.xyz / w.w - u.cam.xyz);
    let c = sky_color(dir);
    return vec4<f32>(vignette(c, i.clip.xy), 1.0);
}

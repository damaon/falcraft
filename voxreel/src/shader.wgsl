struct U {
    view_proj: mat4x4<f32>,
    light_vp: mat4x4<f32>,
    inv_view_proj: mat4x4<f32>,
    sun: vec4<f32>,
    cam: vec4<f32>,
    sky_top: vec4<f32>,
    sky_horizon: vec4<f32>,
    ground: vec4<f32>,
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

fn tonemap(c: vec3<f32>) -> vec3<f32> {
    // Gentle shoulder so glow/bright faces roll off instead of clipping.
    return c * (1.0 + c / 6.0) / (1.0 + c);
}

fn fog(c: vec3<f32>, world: vec3<f32>) -> vec3<f32> {
    let d = distance(world, u.cam.xyz);
    let f = smoothstep(u.params.x, u.params.y, d);
    return mix(c, u.sky_horizon.rgb, f);
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
    return vec4<f32>(fog(tonemap(c), i.world), 1.0);
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
    // Anti-aliased unit grid that fades out with distance.
    let p = i.world.xz;
    let g = abs(fract(p - 0.5) - 0.5) / fwidth(p);
    let line = 1.0 - min(min(g.x, g.y), 1.0);
    let base = u.ground.rgb * (1.0 - line * 0.10);
    let c = light(base, vec3<f32>(0.0, 1.0, 0.0), i.world, 1.0);
    return vec4<f32>(fog(tonemap(c), i.world), 1.0);
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
    let c = mix(u.sky_horizon.rgb, u.sky_top.rgb, smoothstep(0.0, 0.55, dir.y));
    return vec4<f32>(c, 1.0);
}

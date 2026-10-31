// Reads the sight mask (see src/sight/render.rs), for the sight's own material and the light layer.

// samples spread evenly over a disk of the blur radius (a Vogel spiral)
const SAMPLES: u32 = 16u;
const GOLDEN_ANGLE: f32 = 2.39996323;

/// The mask at `world_position`, 0 beyond it. `area` is the bottom-left corner (xy) and size (zw)
/// of the area the mask covers, in world space.
fn sample(
    mask: texture_2d<f32>,
    mask_sampler: sampler,
    area: vec4<f32>,
    world_position: vec2<f32>,
) -> f32 {
    let local = (world_position - area.xy) / area.zw;
    // the texture's rows go from the top down
    let uv = vec2(local.x, 1.0 - local.y);
    // sampled outside of any branch, as texture samples need uniform control flow
    let sampled = textureSample(mask, mask_sampler, clamp(uv, vec2(0.0), vec2(1.0))).r;
    let inside = all(uv >= vec2(0.0)) && all(uv <= vec2(1.0));
    return select(0.0, sampled, inside);
}

/// How visible `world_position` is, the mask blurred over `blur` (radius in world space).
fn visibility(
    mask: texture_2d<f32>,
    mask_sampler: sampler,
    area: vec4<f32>,
    blur: f32,
    world_position: vec2<f32>,
) -> f32 {
    var visibility = 0.0;
    for (var i = 0u; i < SAMPLES; i++) {
        let radius = blur * sqrt((f32(i) + 0.5) / f32(SAMPLES));
        let angle = f32(i) * GOLDEN_ANGLE;
        visibility += sample(
            mask,
            mask_sampler,
            area,
            world_position + radius * vec2(cos(angle), sin(angle)),
        );
    }
    return visibility / f32(SAMPLES);
}

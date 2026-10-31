// Reads the sight mask (see src/sight/render.rs), for the sight's own material and the light layer.
// The mask is blurred on the CPU, once per cell, so one linearly filtered sample per pixel is enough.

/// How visible `world_position` is, 0 beyond the mask. `area` is the bottom-left corner (xy) and
/// size (zw) of the area the mask covers, in world space.
fn visibility(
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

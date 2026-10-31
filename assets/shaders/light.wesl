// The light layer above everything (see src/light/render.rs): darkens by the night and the sun's
// shadows within the sight, and is lifted where the lightmap is lit.

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
import super::sight_mask;

struct LightMaterial {
    // how much the night darkens, from 0 (not at all) to 1 (black)
    darkness: f32,
    // how much the sun's shadows darken, 0 without sun
    shadow: f32,
    // bottom-left corner (xy) and size (zw) of the shadow mask in world space
    shadow_area: vec4<f32>,
    // bottom-left corner (xy) and size (zw) of the lightmap in world space
    lightmap_area: vec4<f32>,
    // bottom-left corner (xy) and size (zw) of the sight mask in world space
    sight_area: vec4<f32>,
    // radius in world space over which the sight mask is blurred
    sight_blur: f32,
};

// distance in px of the samples of the shadow mask around each pixel, which soften the edges of
// the shadows beyond the mask's linear filtering
const SOFTNESS: f32 = 1.5;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: LightMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var shadow_mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var shadow_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var lightmap: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(4) var lightmap_sampler: sampler;
@group(constants::MATERIAL_BIND_GROUP) @binding(5) var sight_mask_texture: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(6) var sight_sampler: sampler;

/// Texture coordinates of `world_position` in a texture covering `area`, and whether it lies
/// inside.
fn uv(world_position: vec2<f32>, area: vec4<f32>) -> vec3<f32> {
    let local = (world_position - area.xy) / area.zw;
    // the texture's rows go from the top down
    let uv = vec2(local.x, 1.0 - local.y);
    let inside = all(uv >= vec2(0.0)) && all(uv <= vec2(1.0));
    return vec3(clamp(uv, vec2(0.0), vec2(1.0)), select(0.0, 1.0, inside));
}

/// The shadow mask at `world_position`, 0 beyond it.
fn shadow(world_position: vec2<f32>) -> f32 {
    let uv = uv(world_position, material.shadow_area);
    // sampled outside of any branch, as texture samples need uniform control flow
    return textureSample(shadow_mask, shadow_sampler, uv.xy).r * uv.z;
}

/// The lightmap at `world_position`, 0 beyond it.
fn light(world_position: vec2<f32>) -> f32 {
    let uv = uv(world_position, material.lightmap_area);
    return textureSample(lightmap, lightmap_sampler, uv.xy).r * uv.z;
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let position = in.world_position.xy;
    let lit = light(position);
    let shade = (shadow(position + vec2(-SOFTNESS, -SOFTNESS))
        + shadow(position + vec2(SOFTNESS, -SOFTNESS))
        + shadow(position + vec2(-SOFTNESS, SOFTNESS))
        + shadow(position + vec2(SOFTNESS, SOFTNESS))) / 4.0;
    // outside the sight, the darkening of the sight takes the place of the shadows, blurred the
    // same way
    let seen = sight_mask::visibility(
        sight_mask_texture,
        sight_sampler,
        material.sight_area,
        material.sight_blur,
        position,
    );

    let brightness = (1.0 - material.darkness) * (1.0 - material.shadow * shade * seen) + lit;
    return vec4(0.0, 0.0, 0.0, clamp(1.0 - brightness, 0.0, 1.0));
}

// The light layer above everything (see src/light/render.rs): darkens by the night and the sun's
// shadows, and is lifted where the lightmap is lit.

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;

struct LightMaterial {
    // how much the night darkens, from 0 (not at all) to 1 (black)
    darkness: f32,
    // how much the sun's shadows darken, 0 without sun
    shadow: f32,
    // direction towards the sun in world space, of unit length
    sun_direction: vec2<f32>,
    // how far the sun's rays rise per px towards it
    sun_slope: f32,
    // the highest shadow caster in px
    max_height: f32,
    // bottom-left corner (xy) and size (zw) of the heights in world space
    heights_area: vec4<f32>,
    // bottom-left corner (xy) and size (zw) of the lightmap in world space
    lightmap_area: vec4<f32>,
};

// edge length of a cell of the heights in px (cells::CELL_SIZE)
const CELL_SIZE: f32 = 4.0;
// the shadows are searched in steps of half a cell up to 64 px
const STEP: f32 = 2.0;
const STEPS: u32 = 32u;
// distance in px of the samples around each pixel, which soften the shadows' edges
const SOFTNESS: f32 = 1.5;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: LightMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var heights: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var lightmap: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(3) var lightmap_sampler: sampler;

/// Height in px of the shadow caster at `world_position`, 0 if there is none.
fn height(world_position: vec2<f32>) -> f32 {
    let cell = vec2<i32>(floor((world_position - material.heights_area.xy) / CELL_SIZE));
    let size = vec2<i32>(textureDimensions(heights));
    // the texture's rows go from the top down
    let texel = vec2(cell.x, size.y - 1 - cell.y);
    if any(texel < vec2(0)) || any(texel >= size) {
        return 0.0;
    }
    return textureLoad(heights, texel, 0).r * 255.0;
}

/// 1 if something higher than the sun's ray towards it stands between `world_position` and the
/// sun, 0 otherwise. The tops of the shadow casters themselves are lit.
fn in_shadow(world_position: vec2<f32>) -> f32 {
    if height(world_position) > 0.0 {
        return 0.0;
    }
    for (var i = 1u; i <= STEPS; i++) {
        let distance = f32(i) * STEP;
        let ray = distance * material.sun_slope;
        if ray >= material.max_height {
            break;
        }
        if height(world_position + material.sun_direction * distance) > ray {
            return 1.0;
        }
    }
    return 0.0;
}

/// The lightmap at `world_position`, 0 beyond it.
fn light(world_position: vec2<f32>) -> f32 {
    let local = (world_position - material.lightmap_area.xy) / material.lightmap_area.zw;
    // the texture's rows go from the top down
    let uv = vec2(local.x, 1.0 - local.y);
    // sampled outside of any branch, as texture samples need uniform control flow
    let sampled = textureSample(lightmap, lightmap_sampler, clamp(uv, vec2(0.0), vec2(1.0))).r;
    let inside = all(uv >= vec2(0.0)) && all(uv <= vec2(1.0));
    return select(0.0, sampled, inside);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let position = in.world_position.xy;
    let lit = light(position);

    var shadow = 0.0;
    if material.shadow > 0.0 {
        shadow = (in_shadow(position + vec2(-SOFTNESS, -SOFTNESS))
            + in_shadow(position + vec2(SOFTNESS, -SOFTNESS))
            + in_shadow(position + vec2(-SOFTNESS, SOFTNESS))
            + in_shadow(position + vec2(SOFTNESS, SOFTNESS))) / 4.0;
    }

    let brightness = (1.0 - material.darkness) * (1.0 - material.shadow * shadow) + lit;
    return vec4(0.0, 0.0, 0.0, clamp(1.0 - brightness, 0.0, 1.0));
}

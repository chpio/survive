// Shows a color only where the sight mask is visible, or only where it isn't (see
// src/sight/render.rs).

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;

@if(TONEMAP_IN_SHADER)
import bevy_core_pipeline::tonemapping;
@if(TONEMAP_IN_SHADER)
import bevy_sprite_render::mesh2d::view_bindings::view;
@if(SRGB_OUTPUT)
import bevy_render::color_operations::linear_to_srgb;

struct SightMaterial {
    color: vec4<f32>,
    // bottom-left corner (xy) and size (zw) of the area the mask covers, in world space
    mask_area: vec4<f32>,
    // 0 to show the color where the mask is visible, 1 where it isn't
    invert: f32,
    // radius in world space over which the mask is blurred
    blur: f32,
};

// samples spread evenly over a disk of the blur radius (a Vogel spiral)
const SAMPLES: u32 = 16u;
const GOLDEN_ANGLE: f32 = 2.39996323;

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: SightMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var mask_sampler: sampler;

/// The mask at `world_position`, 0 beyond it.
fn sample_mask(world_position: vec2<f32>) -> f32 {
    let local = (world_position - material.mask_area.xy) / material.mask_area.zw;
    // the texture's rows go from the top down
    let uv = vec2(local.x, 1.0 - local.y);
    // sampled outside of any branch, as texture samples need uniform control flow
    let sampled = textureSample(mask, mask_sampler, clamp(uv, vec2(0.0), vec2(1.0))).r;
    let inside = all(uv >= vec2(0.0)) && all(uv <= vec2(1.0));
    return select(0.0, sampled, inside);
}

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    var visibility = 0.0;
    for (var i = 0u; i < SAMPLES; i++) {
        let radius = material.blur * sqrt((f32(i) + 0.5) / f32(SAMPLES));
        let angle = f32(i) * GOLDEN_ANGLE;
        visibility += sample_mask(in.world_position.xy + radius * vec2(cos(angle), sin(angle)));
    }
    visibility /= f32(SAMPLES);
    let alpha = mix(visibility, 1.0 - visibility, material.invert);

    var color = vec4(material.color.rgb, material.color.a * alpha);
@if(TONEMAP_IN_SHADER)
    color = tonemapping::tone_mapping(color, view.color_grading);
@if(SRGB_OUTPUT)
    color = vec4(linear_to_srgb(color.rgb), color.a);
    return color;
}

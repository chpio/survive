// Shows a color only where the sight mask is visible, or only where it isn't (see
// src/sight/render.rs).

import bevy_sprite_render::mesh2d::vertex_output::VertexOutput;
import bevy_sprite_render::mesh2d::functions;
import super::sight_mask;

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
};

@group(constants::MATERIAL_BIND_GROUP) @binding(0) var<uniform> material: SightMaterial;
@group(constants::MATERIAL_BIND_GROUP) @binding(1) var mask: texture_2d<f32>;
@group(constants::MATERIAL_BIND_GROUP) @binding(2) var mask_sampler: sampler;

@fragment
fn fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    // a mesh tag anchors the whole mesh at one point, e.g. a figure at its feet
    // (figure::anchor_tag): x and y offset by 2^15 in its low and high half
    let tag = functions::get_tag(in.instance_index);
    var position = in.world_position.xy;
    if tag != 0u {
        position = vec2<f32>(f32(tag & 0xffffu), f32(tag >> 16u)) - 32768.0;
    }
    let visibility = sight_mask::visibility(
        mask,
        mask_sampler,
        material.mask_area,
        position,
    );
    let alpha = mix(visibility, 1.0 - visibility, material.invert);

    var color = vec4(material.color.rgb, material.color.a * alpha);
@if(TONEMAP_IN_SHADER)
    color = tonemapping::tone_mapping(color, view.color_grading);
@if(SRGB_OUTPUT)
    color = vec4(linear_to_srgb(color.rgb), color.a);
    return color;
}

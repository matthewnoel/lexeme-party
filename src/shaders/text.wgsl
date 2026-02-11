struct ScreenUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

@group(1) @binding(0)
var text_tex: texture_2d<f32>;
@group(1) @binding(1)
var text_sampler: sampler;

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    let clip_x = input.pos.x / (screen.screen_size.x * 0.5) - 1.0;
    let clip_y = 1.0 - input.pos.y / (screen.screen_size.y * 0.5);
    out.clip_pos = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.uv = input.uv;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return textureSample(text_tex, text_sampler, input.uv);
}

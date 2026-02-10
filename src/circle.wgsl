struct ScreenUniform {
    screen_size: vec2<f32>,
    _pad: vec2<f32>,
};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

struct VsIn {
    @location(0) unit_pos: vec2<f32>,
    @location(1) center: vec2<f32>,
    @location(2) radius: f32,
    @location(3) color: vec3<f32>,
};

struct VsOut {
    @builtin(position) clip_pos: vec4<f32>,
    @location(0) color: vec3<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    let world = input.center + input.unit_pos * input.radius;
    let clip_x = world.x / (screen.screen_size.x * 0.5);
    let clip_y = -world.y / (screen.screen_size.y * 0.5);

    var out: VsOut;
    out.clip_pos = vec4<f32>(clip_x, clip_y, 0.0, 1.0);
    out.color = input.color;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    return vec4<f32>(input.color, 1.0);
}

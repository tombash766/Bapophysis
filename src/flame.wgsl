struct Transform { row0: vec4<f32>, row1: vec4<f32>, variation: vec4<f32> }
struct Params { camera: vec4<f32>, size: vec4<u32>, palette: array<vec4<f32>,3>, transforms: array<Transform,4> }
struct Particle { position: vec2<f32>, rng: u32, age: u32, colour: vec4<f32> }
struct Pixel { hits: atomic<u32>, red: atomic<u32>, green: atomic<u32>, blue: atomic<u32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> histogram: array<Pixel>;
fn random(state: u32) -> u32 {
    var x=state; x ^= x<<13u; x ^= x>>17u; x ^= x<<5u; return x;
}
fn variation(p: vec2<f32>, kind: u32) -> vec2<f32> {
    let r2=max(dot(p,p),0.03); let r=sqrt(r2);
    switch kind {
        case 1u: { return sin(p); }
        case 2u: { return p/r2; }
        case 3u: { return vec2<f32>(p.x*sin(r2)-p.y*cos(r2),p.x*cos(r2)+p.y*sin(r2)); }
        case 4u: { return vec2<f32>((p.x-p.y)*(p.x+p.y),2.0*p.x*p.y)/r; }
        default: { return p; }
    }
}
@compute @workgroup_size(128)
fn main(@builtin(global_invocation_id) id: vec3<u32>) {
    if id.x>=arrayLength(&particles) { return; }
    var p=particles[id.x];
    for(var i=0u;i<32u;i++) {
        p.rng=random(p.rng); let r=f32(p.rng>>8u)/16777216.0;
        var index=3u;
        for(var j=0u;j<4u;j++) { if r<params.transforms[j].row1.w { index=j; break; } }
        let t=params.transforms[index];
        let affine=vec2<f32>(dot(t.row0.xy,p.position)+t.row0.z,dot(t.row1.xy,p.position)+t.row1.z);
        let next=mix(affine,variation(affine,u32(t.variation.x)),t.variation.y);
        if !all(abs(next)<vec2<f32>(10000.0)) {
            p.position=vec2<f32>(0.0); p.age=0u; p.colour.x=0.5; continue;
        }
        p.position=next; p.colour.x=(p.colour.x+t.variation.z)*0.5;
        p.age=min(p.age+1u,32u); if p.age<20u { continue; }
        let uv=(next-params.camera.xy)/params.camera.zw+0.5;
        if all(uv>=vec2<f32>(0.0)) && all(uv<vec2<f32>(1.0)) {
            let x=u32(uv.x*f32(params.size.x));
            let y=params.size.y-1u-u32(uv.y*f32(params.size.y));
            let pixel=y*params.size.x+x;
            let c=clamp(p.colour.x*2.0,0.0,2.0);
            let rgb=mix(params.palette[0].xyz,params.palette[1].xyz,min(c,1.0));
            let colour=mix(rgb,params.palette[2].xyz,max(c-1.0,0.0));
            let fixed=vec3<u32>(clamp(colour,vec3<f32>(0.0),vec3<f32>(1.0))*63.0);
            atomicAdd(&histogram[pixel].hits,1u);
            atomicAdd(&histogram[pixel].red,fixed.x);
            atomicAdd(&histogram[pixel].green,fixed.y);
            atomicAdd(&histogram[pixel].blue,fixed.z);
        }
    }
    particles[id.x]=p;
}

struct Transform { row0: vec4<f32>, row1: vec4<f32>, variation: vec4<f32>, parameters:vec4<f32> }
struct Params { camera: vec4<f32>, size: vec4<u32>, palette: array<vec4<f32>,256>, transforms: array<Transform,4> }
struct Particle { position: vec2<f32>, rng: u32, age: u32, colour: vec4<f32> }
struct Pixel { hits: atomic<u32>, red: atomic<u32>, green: atomic<u32>, blue: atomic<u32> }
@group(0) @binding(0) var<uniform> params: Params;
@group(0) @binding(1) var<storage, read_write> particles: array<Particle>;
@group(0) @binding(2) var<storage, read_write> histogram: array<Pixel>;
fn random(state: u32) -> u32 {
    var x=state; x ^= x<<13u; x ^= x>>17u; x ^= x<<5u; return x;
}
fn variation(p: vec2<f32>, t:Transform, noise:vec4<f32>, extra:f32) -> vec2<f32> {
    let kind=u32(t.variation.x);let q=t.parameters;
    let gaussian=dot(noise,vec4<f32>(1.0))-2.0;let tau=6.283185307179586;
    let r2=max(dot(p,p),0.03); let r=sqrt(r2);
    let radius=length(p);let safe_r=max(radius,0.000001);
    var angle=0.0;if radius>0.000001 {angle=atan2(p.x,p.y);}
    let pi=3.141592653589793;
    switch kind {
        case 1u: { return sin(p); }
        case 2u: { return p/r2; }
        case 3u: { return vec2<f32>(p.x*sin(r2)-p.y*cos(r2),p.x*cos(r2)+p.y*sin(r2)); }
        case 4u: { return vec2<f32>((p.x-p.y)*(p.x+p.y),2.0*p.x*p.y)/r; }
        case 5u: {return vec2<f32>(angle/pi,radius-1.0);}
        case 6u: {return radius*vec2<f32>(sin(angle+radius),cos(angle-radius));}
        case 7u: {return radius*vec2<f32>(sin(angle*radius),-cos(angle*radius));}
        case 8u: {return angle/pi*vec2<f32>(sin(pi*radius),cos(pi*radius));}
        case 9u: {return vec2<f32>(cos(angle)+sin(radius),sin(angle)-cos(radius))/safe_r;}
        case 10u: {return vec2<f32>(sin(angle)/safe_r,radius*cos(angle));}
        case 11u: {return vec2<f32>(sin(angle)*cos(radius),cos(angle)*sin(radius));}
        case 12u: {let a=sin(angle+radius);let b=cos(angle-radius);return radius*vec2<f32>(a*a*a+b*b*b,a*a*a-b*b*b);}
        case 13u: {return vec2<f32>(select(p.x,2.0*p.x,p.x<0.0),select(p.y,0.5*p.y,p.y<0.0));}
        case 14u: {return 2.0*p.yx/(radius+1.0);}
        case 15u: {return 2.0*p/(radius+1.0);}
        case 16u: {return 4.0*p/(dot(p,p)+4.0);}
        case 17u: {return vec2<f32>(sin(p.x),p.y);}
        case 18u: {let a=angle*0.5+select(0.0,pi,noise.x>=0.5);return sqrt(radius)*vec2<f32>(cos(a),sin(a));}
        case 19u: {return p+q.xy*sin(p.yx/(q.zw*q.zw+vec2<f32>(0.000001)));}
        case 20u: {return p+q.xy*sin(tan(3.0*p.yx));}
        case 21u: {return exp(clamp(p.x-1.0,-40.0,40.0))*vec2<f32>(cos(pi*p.y),sin(pi*p.y));}
        case 22u: {return pow(safe_r,sin(angle))*vec2<f32>(cos(angle),sin(angle));}
        case 23u: {let y=clamp(p.y,-40.0,40.0);let a=exp(y);let b=exp(-y);return vec2<f32>(cos(pi*p.x)*(a+b)*0.5,-sin(pi*p.x)*(a-b)*0.5);}
        case 24u: {let d=t.row0.z*t.row0.z+0.000001;let rr=(radius+d)%(2.0*d)-d+radius*(1.0-d);return rr*vec2<f32>(cos(angle),sin(angle));}
        case 25u: {let d=pi*(t.row0.z*t.row0.z+0.000001);let a=angle+select(d*0.5,-d*0.5,(angle+t.row1.z)%d>d*0.5);return radius*vec2<f32>(cos(a),sin(a));}
        case 26u: {let a=noise.x*tau;return p*noise.y*vec2<f32>(cos(a),sin(a));}
        case 27u: {let a=noise.x*tau;return noise.y*vec2<f32>(cos(a),sin(a));}
        case 28u: {let a=extra*tau;return gaussian*vec2<f32>(cos(a),sin(a));}
        case 29u: {return p;}
        case 30u: {let a=atan2(p.y,p.x)+sin(q.x*pi*0.5)*gaussian;let z=cos(q.x*pi*0.5)*gaussian-1.0;return radius*vec2<f32>(cos(a),sin(a))+z*p;}
        case 31u: {let d=q.x*q.x+0.000001;let rr=radius-2.0*d*trunc((radius+d)/(2.0*d))+radius*(1.0-d);return rr*vec2<f32>(sin(angle),cos(angle));}
        case 32u: {let d=pi*(q.x*q.x+0.000001);let f=angle+q.y-d*trunc((angle+q.y)/d);let a=angle+select(d*0.5,-d*0.5,f>d*0.5);return radius*vec2<f32>(sin(a),cos(a));}
        case 33u: {let rr=radius*(q.x+(q.y-q.x)*(0.5+0.5*sin(q.z*angle)));return rr*vec2<f32>(sin(angle),cos(angle));}
        case 34u: {return vec2<f32>(sin(q.x*p.y)-cos(q.y*p.x),sin(q.z*p.x)-cos(q.w*p.y));}
        case 35u: {let a=q.x*pi*0.5;let raw=q.y-p.y*sin(a);let d=select(raw,0.000001,abs(raw)<0.000001);return q.y*vec2<f32>(p.x,cos(a)*p.y)/d;}
        case 36u,37u: {let power=max(round(abs(q.x)),1.0);let branch=floor(noise.x*power);let theta=atan2(p.y,p.x)*select(1.0,-1.0,kind==37u && u32(branch)%2u==1u);let a=(theta+tau*branch)/power;return pow(safe_r,q.y/power)*vec2<f32>(cos(a),sin(a));}
        case 38u: {let re=1.0+q.x*p.x+q.y*(p.x*p.x-p.y*p.y);let im=q.x*p.y+2.0*q.y*p.x*p.y;return vec2<f32>(p.x*re+p.y*im,p.y*re-p.x*im)/max(re*re+im*im,0.000001);}
        case 39u: {let d=max(abs(q.xy),vec2<f32>(0.000001));return (2.0*floor(p/d)+1.0)*d-p;}
        default: { return p; }
    }
}
fn transform_point(point:vec2<f32>,t:Transform,noise:vec4<f32>,extra:f32)->vec2<f32> {
    var p=point;
    if u32(t.variation.x)==29u {
        let a=extra*6.283185307179586;let amount=t.parameters.x*(dot(noise,vec4<f32>(1.0))-2.0);
        p+=amount*vec2<f32>(cos(a),sin(a));
    }
    let affine=vec2<f32>(dot(t.row0.xy,p)+t.row0.z,dot(t.row1.xy,p)+t.row1.z);
    return mix(affine,variation(affine,t,noise,extra),t.variation.y);
}
// Verification entry point: compare every shader formula against the CPU probe.
@compute @workgroup_size(128)
fn check_variations(@builtin(global_invocation_id) id:vec3<u32>) {
    if id.x<80u {
        var t=params.transforms[0];t.variation.x=f32(id.x%40u);t.variation.y=1.0;
        var noise=vec4<f32>(0.17,0.31,0.53,0.83);var extra=0.23;
        if id.x>=40u {noise=vec4<f32>(0.57,0.72,0.66,0.44);extra=0.71;}
        particles[id.x].position=transform_point(vec2<f32>(0.7,-0.4),t,noise,extra);
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
        var noise:array<f32,5>;
        for(var k=0u;k<5u;k++){p.rng=random(p.rng);noise[k]=f32(p.rng>>8u)/16777216.0;}
        let next=transform_point(p.position,t,vec4<f32>(noise[0],noise[1],noise[2],noise[3]),noise[4]);
        if !all(abs(next)<vec2<f32>(10000.0)) {
            p.position=vec2<f32>(0.0); p.age=0u; p.colour.x=0.5; continue;
        }
        p.position=next; p.colour.x=(p.colour.x+t.variation.z)*0.5;
        p.age=min(p.age+1u,32u); if p.age<20u { continue; }
        let uv=(next-params.camera.xy)/params.camera.zw+0.5;
        // Accumulate the current rectangular camera view; there is no radial mask.
        if all(uv>=vec2<f32>(0.0)) && all(uv<vec2<f32>(1.0)) {
            let x=u32(uv.x*f32(params.size.x));
            let y=params.size.y-1u-u32(uv.y*f32(params.size.y));
            let pixel=y*params.size.x+x;
            let c=clamp(p.colour.x,0.0,1.0)*255.0;
            let lower=min(u32(c),254u);
            let colour=mix(params.palette[lower].xyz,params.palette[lower+1u].xyz,c-f32(lower));
            let fixed=vec3<u32>(clamp(colour,vec3<f32>(0.0),vec3<f32>(1.0))*63.0);
            atomicAdd(&histogram[pixel].hits,1u);
            atomicAdd(&histogram[pixel].red,fixed.x);
            atomicAdd(&histogram[pixel].green,fixed.y);
            atomicAdd(&histogram[pixel].blue,fixed.z);
        }
    }
    particles[id.x]=p;
}

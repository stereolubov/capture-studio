// Рендер egui: вершины в логических точках, цвет — sRGB с premultiplied alpha.
// Смешивание выполняется в гамма-пространстве (как в браузере и в egui-эталоне),
// поэтому цели рендера и текстуры — *_UNORM, а не *_UNORM_SRGB.

cbuffer Constants : register(b0)
{
    float2 screen_size_points;
    float2 padding;
};

struct VsIn
{
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

struct PsIn
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
    float4 color : COLOR0;
};

PsIn vs_main(VsIn input)
{
    PsIn o;
    o.pos = float4(2.0 * input.pos.x / screen_size_points.x - 1.0,
                   1.0 - 2.0 * input.pos.y / screen_size_points.y,
                   0.0, 1.0);
    o.uv = input.uv;
    o.color = input.color;
    return o;
}

Texture2D tex0 : register(t0);
SamplerState sampler0 : register(s0);

float4 ps_main(PsIn input) : SV_Target
{
    return input.color * tex0.Sample(sampler0, input.uv);
}

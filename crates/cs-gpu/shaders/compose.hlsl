// Компоновщик сцены (§7.3): текстурированные и сплошные прямоугольники на холсте.
// Вершины уже в NDC (CPU считает углы с учётом поворота и обрезки); цвет — premultiplied.

cbuffer Item : register(b0)
{
    float4 color;    // для сплошной заливки; для текстуры — множитель (opacity в .a и .rgb)
    float textured;  // 1 — брать цвет из текстуры
    float3 padding;
};

struct VsIn
{
    float2 pos : POSITION;
    float2 uv : TEXCOORD0;
};

struct PsIn
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
};

PsIn vs_main(VsIn input)
{
    PsIn o;
    o.pos = float4(input.pos, 0.0, 1.0);
    o.uv = input.uv;
    return o;
}

Texture2D tex0 : register(t0);
SamplerState sampler0 : register(s0);

float4 ps_main(PsIn input) : SV_Target
{
    if (textured > 0.5)
        return tex0.Sample(sampler0, input.uv) * color;
    return color;
}

// Конвертер цвета (§7.3): холст RGBA (sRGB-гамма) → NV12 (8 бит) для энкодера.
// Плоскость Y рисуется в R8 (полный размер), плоскость UV — в R8G8 (половина размера).
// Масштабирование до выходного разрешения — билинейной выборкой при отрисовке.

cbuffer Params : register(b0)
{
    float4 y_coef;   // (Kr', Kg', Kb', offset) для Y
    float4 u_coef;   // для U (Cb)
    float4 v_coef;   // для V (Cr)
    float2 texel;    // размер текселя источника (1/ширина, 1/высота)
    float2 padding;
};

Texture2D canvas : register(t0);
SamplerState linear_clamp : register(s0);

struct PsIn
{
    float4 pos : SV_POSITION;
    float2 uv : TEXCOORD0;
};

// Полноэкранный треугольник без вершинного буфера.
PsIn vs_main(uint id : SV_VertexID)
{
    PsIn o;
    float2 uv = float2((id << 1) & 2, id & 2);
    o.uv = uv;
    o.pos = float4(uv * float2(2.0, -2.0) + float2(-1.0, 1.0), 0.0, 1.0);
    return o;
}

float ps_y(PsIn i) : SV_Target
{
    float3 rgb = canvas.Sample(linear_clamp, i.uv).rgb;
    return dot(rgb, y_coef.rgb) + y_coef.a;
}

float2 ps_uv(PsIn i) : SV_Target
{
    // Среднее по 2×2 пикселям выхода: выборка в центре блока цветности.
    float3 rgb = canvas.Sample(linear_clamp, i.uv).rgb;
    return float2(dot(rgb, u_coef.rgb) + u_coef.a, dot(rgb, v_coef.rgb) + v_coef.a);
}

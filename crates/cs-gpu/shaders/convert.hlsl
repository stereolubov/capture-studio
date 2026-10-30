// Конвертер цвета (§7.3): холст RGBA (sRGB-гамма) → NV12 (8 бит) или P010 (10 бит).
// Плоскость Y рисуется в R8/R16 (полный размер), плоскость UV — в R8G8/R16G16 (половина).
// Масштабирование: билинейной выборкой при отрисовке либо заранее, ps_scale (bicubic/Lanczos).

cbuffer Params : register(b0)
{
    float4 y_coef;   // (Kr', Kg', Kb', offset) для Y
    float4 u_coef;   // для U (Cb)
    float4 v_coef;   // для V (Cr)
    float2 texel;    // размер текселя источника (1/ширина, 1/высота)
    float quant;     // 0 — 8 бит; 1023 — P010: 10 бит в старших разрядах 16-битного слова
    float padding;
};

// Масштабирование разделимым ядром: проход по одной оси (axis = (1,0) или (0,1)).
cbuffer ScaleParams : register(b1)
{
    float2 axis;
    float2 src_size;
    float scale;     // растяжение ядра: при уменьшении в N раз — N (без муара), иначе 1
    float taps;      // отсчётов ядра по каждую сторону
    float kind;      // 0 — bicubic (Catmull-Rom), 1 — Lanczos-3
    float scale_pad;
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

// P010: 10-битное значение в старших разрядах (младшие 6 — нули), как ждут энкодеры.
float2 quantize(float2 v)
{
    return quant > 0.0 ? round(saturate(v) * quant) * 64.0 / 65535.0 : v;
}

float ps_y(PsIn i) : SV_Target
{
    float3 rgb = canvas.Sample(linear_clamp, i.uv).rgb;
    return quantize(dot(rgb, y_coef.rgb) + y_coef.a).x;
}

float2 ps_uv(PsIn i) : SV_Target
{
    // Среднее по 2×2 пикселям выхода: выборка в центре блока цветности.
    float3 rgb = canvas.Sample(linear_clamp, i.uv).rgb;
    return quantize(float2(dot(rgb, u_coef.rgb) + u_coef.a, dot(rgb, v_coef.rgb) + v_coef.a));
}

static const float PI = 3.14159265;

float kernel_weight(float x)
{
    x = abs(x);
    if (kind < 0.5)
    {
        // Catmull-Rom (B = 0, C = 0.5): резкий, без размытия, слабый «звон».
        if (x < 1.0) return (1.5 * x - 2.5) * x * x + 1.0;
        if (x < 2.0) return ((-0.5 * x + 2.5) * x - 4.0) * x + 2.0;
        return 0.0;
    }
    if (x < 1e-5) return 1.0;
    if (x >= 3.0) return 0.0;
    float px = PI * x;
    return 3.0 * sin(px) * sin(px / 3.0) / (px * px);
}

float4 ps_scale(PsIn i) : SV_Target
{
    // Позиция в текселях источника (центры текселей — целые числа).
    float2 pos = i.uv * src_size - 0.5;
    float p = dot(pos, axis);
    float base = floor(p);
    int2 other = int2(round(pos * (1.0 - axis)));
    int2 limit = int2(src_size) - 1;
    float4 acc = 0.0;
    float sum = 0.0;
    int n = (int)taps;
    [loop]
    for (int k = 1 - n; k <= n; k++)
    {
        float t = base + k;
        float w = kernel_weight((t - p) / scale);
        int2 c = clamp(other + int2(axis * t), int2(0, 0), limit);
        acc += w * canvas.Load(int3(c, 0));
        sum += w;
    }
    return acc / sum;
}

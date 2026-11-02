// [basic.extended.fp], [expr.const]: _Float16 and the storage-only __fp16 in
// constant evaluation. As in clang 19, every _Float16 operation rounds to half
// (no excess precision in constexpr), so 2048 + 1 + 1 stays 2048; __fp16
// operands promote to float, so 65504 + 65504 does not overflow.
// Verified against clang 19.
//@ expect: compile-ok
constexpr _Float16 a = 0.1f16;
constexpr _Float16 b = a + a + a;
static_assert(b == (_Float16)0.2998046875f);
constexpr _Float16 c = (_Float16)1 / (_Float16)3;
static_assert(c > 0.333f16 && c < 0.3335f16);
constexpr __fp16 h = 65504;
static_assert(h + h > 65504.0f);
static_assert((_Float16)65520.0f == __builtin_inf());
constexpr float f = (_Float16)0.1f;
static_assert(f == 0.0999755859375f);
static_assert(sizeof(_Float16) == 2 && alignof(__fp16) == 2);
constexpr _Float16 m = 2049.0f16 + 1.0f16;
static_assert(m == 2048);
constexpr int i = (int)(_Float16)3.75f;
static_assert(i == 3);
constexpr _Float16 a2 = 2048, b2 = 1;
constexpr _Float16 s = a2 + b2 + b2;
static_assert(s == 2048);
constexpr __fp16 hp = 2.0f; constexpr float hq = hp * 3;
static_assert(hq == 6.0f);

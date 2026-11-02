// __bf16 (std::bfloat16_t, [basic.extended.fp], P1467; WP-361): a 'bfloat'
// in IR. clang 19/22 evaluate __bf16 arithmetic in float on both targets
// (-fbfloat16-excess-precision=standard: fpext, operate, fptrunc once per
// full expression); comparisons operate on bfloat directly.
__bf16 add(__bf16 a, __bf16 b) { return a + b; }
__bf16 mul(__bf16 a, float b) { return a * b; }
double cv(__bf16 x) { return x; }
int toi(__bf16 x) { return x; }
__bf16 fromi(int i) { return i; }
__bf16 fromd(double d) { return d; }
bool lt(__bf16 a, __bf16 b) { return a < b; }
__bf16 g = 1.5f;
__bf16 neg(__bf16 a) { return -a; }
void inc(__bf16 &a) { a += 1; ++a; }
__bf16 chain(__bf16 x, __bf16 y, __bf16 z) { return x * y - z / x + y; }
__bf16 pos(__bf16 x) { return +x; }
float wide(__bf16 x, __bf16 y) { return x + y; }
__bf16 cond(bool c, __bf16 x, __bf16 y) { return (c ? x : y) + x; }
void cassign(__bf16 &a, __bf16 b, double d) { a *= b + b; a -= d; a /= 2; }
__bf16 g2 = (__bf16)0.1f * 3;

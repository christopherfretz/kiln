// [basic.extended.fp] / clang: the storage-only __fp16 promotes to float in
// arithmetic (usual unary conversions, and a compound assignment with an
// integer operand); conversions to/from integers and ++/-- go through float.
float widen(__fp16 *h) { return *h; }
void store(__fp16 *h, float f) { *h = f; }
void hadd(__fp16 *a, __fp16 *b) { *a = *a + *b; }
__fp16 gh = 2.25;
double d(__fp16 *h) { return *h * 2.0; }
int toi(__fp16 *h) { return *h; }
void inc(__fp16 &a) { a += 1; ++a; }
float mix(__fp16 *h, _Float16 f) { return *h + f; }
__fp16 ga = 1.0f16 / 3;
bool tb(__fp16 *h) { return *h; }
void fromi(__fp16 *h, int i, unsigned u) { *h = i; h[1] = u; h[2] = 3; }
void cas(__fp16 *a, __fp16 *b, _Float16 *c, int *i, float x) { *a += *b; *a += *c; *a += *i; *a *= x; *c += *a; *i += *a; }

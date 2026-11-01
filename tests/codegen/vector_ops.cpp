// [WP-156] Operations on vector values (clang ScalarExprEmitter): unary
// and binary operators with scalar splats, comparisons yielding sign-extended
// masks, element access through subscripts (bool vectors via their iN
// storage), list-initialization, casts, __builtin_convertvector and the
// __builtin_reduce_* family (llvm.vector.reduce.*).
typedef int v4i __attribute__((ext_vector_type(4)));
typedef unsigned v4u __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef bool v4b __attribute__((ext_vector_type(4)));
typedef int g4i __attribute__((vector_size(16)));
typedef char g16c __attribute__((vector_size(16)));
v4i add(v4i a, v4i b) { return a + b; }
v4i adds(v4i a) { return a * 3; }
v4i cmp(v4i a, v4i b) { return a < b; }
int sub(v4i a, int i) { return a[i]; }
void set(v4i& a, int i) { a[i] = 7; }
v4i mk(int x) { return v4i{x, 1, 2, 3}; }
v4f cv(v4i a) { return __builtin_convertvector(a, v4f); }
bool all(v4i a) { return __builtin_reduce_and(__builtin_convertvector(a, v4b)); }
int sum(v4i a) { return __builtin_reduce_add(a); }
v4i neg(v4i a) { return ~a; }
v4i sh(v4i a) { return a << 2; }
v4i neg2(v4i a) { return -a; }
v4f negf(v4f a) { return -a; }
v4i lnot(v4i a) { return !a; }
g4i lnotg(g4i a) { return !a; }
v4i cmpf(v4f a, v4f b) { return a == b; }
v4i divu(v4u a, v4u b) { return (v4i)(a / b); }
v4i ls(v4i a) { return 5 - a; }
void ca(v4i& a, int x) { a += x; a *= a; }
g4i gadd(g4i a, g4i b) { return a & b; }
g16c gbc(g4i a) { return (g16c)a; }
v4f splf(float x) { return x; }
v4i shr(v4i a, v4i b) { return a >> b; }
v4u shru(v4u a) { return a >> 1; }
float subf(v4f a) { return a[2]; }
bool subb(v4b a, int i) { return a[i]; }
void setb(v4b& a, int i) { a[i] = true; }
v4b ld(const v4b* p) { return *p; }
v4i ternc(v4i a) { v4i r = a; r[0] = r[1] + r[2]; return r; }
int rmax(v4i a) { return __builtin_reduce_max(a); }
unsigned rminu(v4u a) { return __builtin_reduce_min(a); }
float rmaxf(v4f a) { return __builtin_reduce_max(a); }
int rmul(v4i a) { return __builtin_reduce_mul(a) + __builtin_reduce_or(a) + __builtin_reduce_xor(a); }
v4i cv2(v4f a) { return __builtin_convertvector(a, v4i); }
v4u cv3(v4b a) { return __builtin_convertvector(a, v4u); }
v4b cv4(v4f a) { return __builtin_convertvector(a, v4b); }

// [WP-173] Brace elision of GNU vector members ([dcl.init.aggr]/16; clang
// CheckSubElementType): scalars initialize the vector's elements one by
// one, both in constant initializers and in run-time aggregate
// initialization; a vector initializer copies the whole member.
typedef int g4i __attribute__((vector_size(16)));
typedef float g2f __attribute__((vector_size(8)));
struct S { g4i a; int b; };
struct F { g2f f; int z; };
S gs = {1, 2, 3, 4, 5};
g4i garr[2] = {1, 2, 3, 4, 5};
S make(int x, int y) { S s = {x, y, x + y, 4, y}; return s; }
F makef(float a, int z) { F f = {a, 2.0f, z}; return f; }
S copy(g4i v) { S s = {v, 7}; return s; }
void arr(int x, g4i *out) { g4i a[2] = {x, x, x, x, 9}; out[0] = a[0]; out[1] = a[1]; }

// [WP-173] The GNU element-wise conditional (clang
// CheckVectorConditionalTypes; ScalarExprEmitter::
// VisitAbstractConditionalOperator): a GNU vector condition selects lanes
// with `icmp ne` + `select`; an ext vector condition selects by each
// lane's sign bit through and/or masks (float lanes bit-cast); scalar arms
// are splatted to the condition's lane count. A scalar condition with
// vector arms branches as usual, after CheckVectorOperands.
typedef int g4i __attribute__((vector_size(16)));
typedef unsigned g4u __attribute__((vector_size(16)));
typedef float g4f __attribute__((vector_size(16)));
typedef int v4i __attribute__((ext_vector_type(4)));
typedef float v4f __attribute__((ext_vector_type(4)));
typedef long long g2ll __attribute__((vector_size(16)));
typedef double g2d __attribute__((vector_size(16)));

g4i gsel(g4i c, g4i a, g4i b) { return c ? a : b; }
g4f gself(g4i c, g4f a, g4f b) { return c ? a : b; }
g4u gselu(g4u c, g4u a) { return c ? a : 7u; }
g4i gscal(g4i c, int x, int y) { return c ? x : y; }
g4f gscalf(g4i c, float x) { return c ? x : 2.0f; }
g4i gmix(g4i c, g4i a, int y) { return c ? a : y; }
g2d gdbl(g2ll c, g2d a, double y) { return c ? a : y; }
g4i gcmp(g4i a, g4i b) { return a < b ? a : b; }
v4i esel(v4i c, v4i a, v4i b) { return c ? a : b; }
v4f eself(v4i c, v4f a, v4f b) { return c ? a : b; }
v4i escal(v4i c, int x) { return c ? x : 0; }
v4f emix(v4i c, v4f a) { return c ? 1.5f : a; }
g4i ssel(bool b, g4i a, g4i c) { return b ? a : c; }
g4i ssplat(int i, g4i a) { return i ? a : 1; }
v4f ssplatf(bool b, v4f a) { return b ? a : 0.5f; }

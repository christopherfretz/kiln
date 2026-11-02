// clang ext_vector_type element access (clang `EmitExtVectorElementExpr`):
// a prvalue base is stored to a temporary; one lane loads with
// `extractelement`, several with a one-operand `shufflevector`; a store
// of one lane is `insertelement`, of several a shuffle into the loaded
// vector (widened first when the vector is longer than the swizzle);
// nested accesses on an object compose their lanes; a constant swizzle
// folds.
typedef float f4 __attribute__((ext_vector_type(4)));
typedef float f2 __attribute__((ext_vector_type(2)));
typedef int i4 __attribute__((ext_vector_type(4)));
typedef int i3 __attribute__((ext_vector_type(3)));
float a(float x) { return ((f4){1, 2, 3, 4}).y + ((f4){x, 2, 3, 4}).x; }
float b(f4 v) { return v.w + v.s2 + v.b; }
f2 c(f4 v) { return v.xy + v.hi + v.odd; }
void d(f4 &v, float x) { v.y = x; v.xz = (f2){x, x}; }
int e(i4 v) { return v.even.y + v.wzyx.lo.x; }
void f(i4 &v, int x) { v.wzyx.y = x; v.zw.yx = (i4){x, 1, 2, 3}.xy; }
i4 g(i4 v) { return v.wzyx; }
void h(i3 &v, int x) { v.xy = (i4){x, 1, 2, 3}.zw; v.z = x; }
int k(i3 v) { return v.z + v.hi.x; }
float m(f4 v) { return (v + v).w; }
constexpr i4 sw = ((i4){1, 2, 3, 4}).wzyx;
i4 gsw = sw;
int q = ((i4){5, 6, 7, 8}).z;

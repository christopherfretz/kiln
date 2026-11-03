// Clang ext_vector_type element access ([clang] "Vectors and Extended
// Vectors"; clang 19 CheckExtVectorComponent): the point (xyzw) and color
// (rgba) sets must not mix, the s/S hex form names lanes by index, hi/lo/
// even/odd halve the vector, a lane past the vector is "exceeds type",
// duplicate lanes are not assignable, a prvalue or const base is not
// assignable, and a scalar result has no members (WP-307).
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 37:15 error "illegal vector component name 'r'"
//@ diag: 38:14 error "illegal vector component name 'q'"
//@ diag: 39:13 error "vector component access exceeds type 'float3' (vector of 3 'float' values)"
//@ diag: 40:23 error "vector is not assignable (contains duplicate components)"
//@ diag: 41:29 error "vector component access exceeds type 'int8' (vector of 8 'int' values)"
//@ diag: 44:21 error "expression is not assignable"
//@ diag: 46:29 error "cannot assign to variable 'cc' with const-qualified type 'const float4' (vector of 4 'float' values)"
//@ diag: 46:16 note "variable 'cc' declared const here"
//@ diag: 53:14 error "illegal vector component name 's'"
//@ diag: 54:15 error "illegal vector component name 'x'"
//@ diag: 55:15 error "illegal vector component name 's'"
//@ diag: 61:15 error "illegal vector component name 'g'"
//@ diag: 64:15 error "member reference base type 'float' is not a structure or union"
//@ diag: 68:53 error "vector is not assignable (contains duplicate components)"
//@ diag: 66:44 error "illegal vector component name 'q'"
//@ diag: 69:20 note "in instantiation of function template specialization 'bad<float __attribute__((ext_vector_type(4)))>' requested here"
//@ diag: 67:40 error "vector is not assignable (contains duplicate components)"
//@ diag: 69:28 note "in instantiation of function template specialization 'dup<float __attribute__((ext_vector_type(4)))>' requested here"
//@ requires: clang>=22
// clang 22 spelling of ext-vector-access-diagnostics.cpp (WP-394).
typedef float float4 __attribute__((ext_vector_type(4)));
typedef float float2 __attribute__((ext_vector_type(2)));
typedef float float3 __attribute__((ext_vector_type(3)));
typedef int int8 __attribute__((ext_vector_type(8)));
constexpr float4 cv = {1, 2, 3, 4};
static_assert(cv.x == 1);
void f(float4 v, float3 t, int8 w, float4 *p) {
  auto a = v.xr;
  auto b = v.q;
  auto c = t.w;
  auto d = v.xx; v.xx = d;
  auto e = w.s7; auto e2 = w.sA;
  auto g = v.hi; auto h = t.odd; auto i = w.even.lo;
  auto j = p->zw;
  float4{1,2,3,4}.x = 3;
  auto k = v.xyzwx;
  const float4 cc = v; cc.x = 1;
  auto l = v.S3;
}
typedef float float16 __attribute__((ext_vector_type(16)));
typedef float float1 __attribute__((ext_vector_type(1)));
void f2(float4 v, float3 t, float16 s, float1 o) {
  auto a = v.rgba;
  auto b = v.s;
  auto c = v.sxy;
  auto d = v.xs;
  auto e = s.sfF0;
  auto g = v.lo.hi;
  auto h = o.hi;
  auto i = v.xyzwxyzwxyzwxyzwx;
  auto j = t.hi;
  auto k = v.Sg;
  auto l = v.ab;
  auto m = (v).x;
  auto n = v.x.x;
}
template<class V> auto bad(V v) { return v.q; }
template<class V> void dup(V v) { v.xx = v.yy; }
template<class T> void nondep(T) { float4 v{}; v.yy = v.xy; }
void g(float4 v) { bad(v); dup(v); nondep(0); }

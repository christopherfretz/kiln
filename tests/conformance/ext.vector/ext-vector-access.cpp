// Clang ext_vector_type element access at run time ([clang] "Vectors and
// Extended Vectors"; clang 19 EmitExtVectorElementExpr): one lane is an
// element, several a shufflevector; a swizzle stores lane by lane, a
// swizzle of a swizzle lvalue composes, `->` reads through the pointer,
// and a prvalue base is swizzled as a value; inside templates, the
// access is formed again on substitution (WP-307).
//@ expect: run
typedef float float4 __attribute__((ext_vector_type(4)));
typedef float float3 __attribute__((ext_vector_type(3)));
typedef float float2 __attribute__((ext_vector_type(2)));
typedef int int8 __attribute__((ext_vector_type(8)));
float4 mk() { return (float4){1, 2, 3, 4}; }
template<class V> auto hi(V v) { return v.hi; }
template<class V> void bump(V *p) { p->x += 10; p->zw = p->yx; ++p->y; }
int run1() {
  float4 v = mk();
  int r = 0;
  if (v.wzyx.x != 4) r |= 1;
  float2 h = hi(v);
  if (h.x != 3 || h.y != 4) r |= 2;
  if (v.lo.y != 2 || v.even.y != 3 || v.odd.x != 2) r |= 4;
  if (mk().zw.y != 4 || mk().y != 2) r |= 8;
  bump(&v);
  if (v.x != 11 || v.y != 3 || v.z != 2 || v.w != 11) r |= 16;
  int8 w = {0, 1, 2, 3, 4, 5, 6, 7};
  if (w.s7 != 7 || w.s01.y != 1 || w.S6 != 6) r |= 32;
  float3 t = {1, 2, 3};
  float2 th = t.hi;
  if (th.x != 3) r |= 64;
  v.xyzw = v.wzyx;
  if (v.x != 11 || v.w != 11 || v.y != 2) r |= 128;
  const float4 c = {5, 6, 7, 8};
  if (c.ww.y != 8) r |= 256;
  return r;
}
float4 g = {1, 2, 3, 4};
float gx = g.y;            // dynamic initialization
float2 gz = g.zw;
int run2() {
  float4 v = {1, 2, 3, 4};
  int r = 0;
  v.wzyx.xy = (float2){9, 8};   // w = 9, z = 8
  if (v.w != 9 || v.z != 8) r |= 1;
  v.xy += v.zw;
  if (v.x != 9 || v.y != 11) r |= 2;
  v.xy *= 2;
  if (v.x != 18 || v.y != 22) r |= 4;
  (v.yx).y = 5;
  if (v.x != 5) r |= 8;
  if (gx != 2 || gz.y != 4) r |= 16;
  float4 *p = &v;
  float2 q = p->zw;
  if (q.x != 8) r |= 32;
  v.x++;
  if (v.x != 6) r |= 64;
  return r;
}
float nt(float4 v) { float2 q = v.xy; return v.x + q.y + v.w; }
template<class V> auto get_x(V v) { return v.x; }
template<class V> auto get_xy(V v) { return v.xy; }
template<class T> T sum(T a) { typedef T v4 __attribute__((ext_vector_type(4))); v4 v = {a, a, a, a}; v.yz = v.xw; return v.x + v.z; }
template<class V> void set(V &v) { v.y = 7; }
int run3() {
  float4 v = {1, 2, 3, 4};
  float2 xy = get_xy(v);
  set(v);
  return (nt(v) == 1 + 7 + 4 && get_x(v) == 1 && xy.y == 2 && v.y == 7 && sum(2.0f) == 4) ? 0 : 1;
}
int main() { return run1() | run2() << 10 | run3() << 20; }

// Clang ext_vector_type element access (clang ext; [expr.ref] shape):
// xyzw / rgba / sN / hi,lo,even,odd component names, length checks at the
// '.', duplicate components making the result non-assignable, unused
// results, and a dependent base checked at instantiation ([temp.res]).
//@ expect: compile-fail
//@ strict-diags
//@ args: -Wno-unused-variable
//@ diag: 31:14 error "illegal vector component name 'q'"
//@ diag: 32:14 error "vector component access exceeds type 'f2' (vector of 2 'float' values)"
//@ diag: 33:11 error "illegal vector component name 'r'"
//@ diag: 34:11 error "vector component access exceeds type 'f4' (vector of 4 'float' values)"
//@ diag: 36:12 error "member reference base type 'gv' (vector of 4 'int' values) is not a structure or union"
//@ diag: 37:14 error "address of vector element requested"
//@ diag: 38:8 error "vector is not assignable (contains duplicate components)"
//@ diag: 39:8 error "cannot assign to variable 'cv' with const-qualified type 'const f4' (vector of 4 'float' values)"
//@ diag: 30:35 note "variable 'cv' declared const here"
//@ diag: 42:14 error "illegal vector component name 's'"
//@ diag: 45:3 warning "expression result unused"
//@ diag: 46:3 warning "expression result unused"
//@ diag: 47:3 warning "left operand of comma operator has no effect"
//@ diag: 47:6 warning "expression result unused"
//@ diag: 49:54 error "illegal vector component name 'q'"
//@ diag: 50:11 note "in instantiation of function template specialization 'get<float __attribute__((ext_vector_type(4)))>' requested here"
//@ requires: clang<22
// clang 22 emits error: illegal vector component name 'q'; WP-355 residual
typedef float f4 __attribute__((ext_vector_type(4)));
typedef float f2 __attribute__((ext_vector_type(2)));
typedef float f3 __attribute__((ext_vector_type(3)));
typedef int gv __attribute__((vector_size(16)));
void t(f4 v, f2 w, gv g, const f4 cv) {
  float a = v.q;
  float b = w.z;
  f2 c = v.xr;
  f2 d = v.s9;
  f3 e = v.xyz;
  int h = g.x;
  float *p = &v.x;
  v.xx = w;
  cv.x = 1;
  f4 f = v.xyzz;
  auto k = v.lo;
  float m = v.s;
  float n = (v + v).y;
  f3 o = v.abg;
  v.x;
  v.even.y;
  1, v.w;
}
template <class T> float get(T t) { return t.zz.y + t.q; }
float u = get(f4{});

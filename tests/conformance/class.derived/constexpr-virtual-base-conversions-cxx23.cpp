// [expr.const]/5, [basic.types.general]/10 before C++26: an object of a
// class with virtual bases cannot be created during constant evaluation,
// so converting a constexpr local to its virtual base never happens; a
// static object's derived-to-virtual-base conversion is fine (the
// complete object is known). P3533R2 (C++26) allows both; see
// constexpr-virtual-base-conversions.cpp.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:15 error "static assertion expression is not an integral constant expression"
//@ diag: 20:5 note "non-literal type 'Y' cannot be used in a constant expression"
//@ diag: 25:15 note "in call to 'paths()'"
//@ requires: clang<22
// clang 22 differs (no literal type check in C++23, P2448): see constexpr-virtual-base-conversions-cxx23-22.cpp
struct V { int v = 1; virtual int f() const { return v; } };
struct W : virtual V { int w = 2; };
struct X : virtual V { int x = 3; };
struct Y : W, X {};
constexpr bool paths() {
  Y y;
  V &v1 = static_cast<W &>(y);
  V &v2 = static_cast<X &>(y);
  return &v1 == &v2;
}
static_assert(paths());
Y gy;
constexpr bool global_paths() {
  V &v1 = static_cast<W &>(gy);
  V &v2 = static_cast<X &>(gy);
  return &v1 == &v2;
}
static_assert(global_paths());
constexpr V *gv = static_cast<W *>(&gy);

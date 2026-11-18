// [temp.func.order]/3 with [over.match.funcs.general]/4: an explicit object
// parameter takes the object argument of a member call, so partial ordering
// compares it like any other parameter; only a member with an implicit
// object parameter gets one inserted. `g(this int)` is more specialized
// than `g(this T)`, and `k(this D &, T *)` than the implicit object member
// `k(T)` (clang 22 and GCC 16.1 agree), also between two bases'
// call operators brought in by using-declarations and with a lambda's
// (found running GCC's C++ testsuite; WP-1519).
//@ std: c++23
//@ expect: compile-ok
//@ strict-diags
struct D {
  template <class T> char g(this T) { return 0; }
  template <class U = void> long g(this int) { return 0; }
  template <class T> char k(T) { return 0; }
  template <class T> long k(this D &, T *) { return 0; }
  operator int() const { return 0; }
};
static_assert(sizeof(D{}.g<int>()) == sizeof(long));
void use_k(D &d, int *p) { static_assert(sizeof(d.k(p)) == sizeof(long)); }
struct E1 { template <class T> char operator()(this T) { return 0; } };
struct E2 { template <class U = void> long operator()(this int) { return 0; } };
struct E : E1, E2 {
  using E1::operator();
  using E2::operator();
  operator int() const { return 0; }
};
static_assert(sizeof(E{}.operator()<int>()) == sizeof(long));
template <class L> struct S : L, E2 {
  using L::operator();
  using E2::operator();
  operator int() const { return 0; }
};
void f() {
  auto s = S{[](this auto) { return '\0'; }};
  static_assert(sizeof(s.operator()<int>()) == sizeof(long));
  auto r = S{[x = 1](this auto &&) { return '\0'; }};
  static_assert(sizeof(r.operator()<int>()) == sizeof(long));
}
struct F {
  template <class T> char operator+(this T, int) { return 0; }
  template <class U = void> long operator+(this F, int) { return 0; }
  template <class T> char operator-(T) { return 0; }
  template <class T> long operator-(this F &, T *) { return 0; }
};
static_assert(sizeof(F{} + 1) == sizeof(long));
void use_minus(F &f, int *p) { static_assert(sizeof(f - p) == sizeof(long)); }

// [temp.res]/[temp.inst], [dcl.init.general]/16.6.2.2 (P0960), [expr.type.conv]/2:
// a parenthesized aggregate initialization `A(a, b)` of a non-dependent
// aggregate whose operands are only instantiation-dependent (locals of a
// template) is analyzed in the pattern and initialized again from the
// substituted operands at instantiation. libc++ test_offset_time_zone.h
// (`std::from_chars_result(end, std::errc{})`). WP-296.
//@ expect: compile-ok

struct R { const char* p; int e; };
union U { int i; long l; };
struct N { R r; int k; };
struct Arr { int a[3]; };

template <int K> constexpr int f(const char* b) {
  const char* end = b + K;
  R r = R(end, K);          // functional cast over the paren-list init
  R s = R(end);             // one operand: the remaining member value-initialized
  U u = U(K);               // a union's first member
  N n = N(R(end, 1), K);    // nested
  int e = 0;
  e += (R(end, K)).e;       // as an operand
  return (r.p == end) + r.e + (s.p == end) * 10 + s.e + u.i * 100 + n.r.e + n.k + e;
}
static_assert(f<2>("abc") == 1 + 2 + 10 + 0 + 200 + 1 + 2 + 2);

constexpr const char* end_of(int) { return nullptr; }
template <class T> constexpr T g(T t) {
  T a = t;
  return Arr({a, a + 1}).a[1] + N({end_of(a), a}, a).r.e;
}
static_assert(g(4) == 5 + 4);

int main() {}

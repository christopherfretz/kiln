// [expr.eq]/2 (P2865): comparing two arrays is ill-formed in C++26. clang
// diagnoses it (`-Warray-compare-cxx26`, a default-error warning) only in
// a template definition, not in an instantiation, and constraint
// satisfaction counts as one: a requires-expression `t == u` with arrays is
// satisfied, as libc++'s test of `equality_comparable<int[5]>` expects
// from clang (std/concepts/concepts.compare/concept.equalitycomparable/
// equality_comparable.compile.pass.cpp).
//@ expect: run
//@ std: c++26
template <class T> concept EQ = requires(T& t, T& u) { t == u; };
static_assert(EQ<int[5]>);

template <class T>
  requires requires(T& t) { t != t; }
constexpr int pick(T&) { return 1; }
constexpr int pick(...) { return 2; }

template <class T> bool same(T& a, T& b) { return a == b; }

int main() {
  int a[5] = {}, b[5] = {};
  if (pick(a) != 1) return 1;
  return same(a, b) ? 2 : 0;
}

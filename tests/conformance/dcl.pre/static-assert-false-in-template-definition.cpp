// [dcl.pre]/10 (P2593R1, a DR): a static_assert-declaration in a template
// definition is not evaluated until instantiation, even when its condition
// is not value-dependent, so `else static_assert(false);` in a discarded
// branch is fine (libstdc++ ranges:9507 `ranges::to`; WP-332). An
// instantiation that keeps the branch fails.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:17 error "static assertion failed: no"
//@ diag: 25:11 note "in instantiation of function template specialization 'deduce<long long>' requested here"
//@ diag: 16:19 error "static assertion failed"
//@ diag: 26:16 note "in instantiation of function template specialization 'pick<char>' requested here"
template<class T> int pick() {
  if constexpr (sizeof(T) > 1)
    return 1;
  else
    static_assert(false);
}
template<class T> int deduce() {
  if constexpr (sizeof(T) > 1) return 1;
  static_assert(false, "no");
}
template<class T> struct Never { static_assert(false); };
template<class T> auto lam = [](auto x) { static_assert(false); return x; };
int ok = pick<long long>();
int bad = deduce<long long>();
int also_bad = pick<char>();

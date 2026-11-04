// [temp.inst]/8, [temp.point]/1, [expr.const]/10.3: a member function of a
// specialization that is used before its pattern is defined, and so waits
// in the end-of-TU queue, is instantiated when a constant expression
// evaluated after the definition needs it (GCC; WP-484). clang 19/22 leave
// it an undefined function for the evaluator. The case of a pattern still
// undefined at the evaluation is temp.inst-constexpr-still-undefined.cpp.
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang leaves a queued specialization undefined for the evaluator
template <class T> struct S { static constexpr int f(); };
struct X : S<int> {
  static constexpr int g() { return f(); }
  static constexpr int h();
};
template <class T> constexpr int S<T>::f() { return 42; }
constexpr int X::h() {
  static_assert(g() == 42);
  static_assert(f() == 42);
  return 0;
}

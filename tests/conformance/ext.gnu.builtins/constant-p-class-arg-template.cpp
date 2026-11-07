// __builtin_constant_p is a custom-checked builtin (clang
// CheckBuiltinFunctionCall): its one argument is not passed through an
// ellipsis, so an object of non-trivial class type is no
// -Wnon-pod-varargs error. That holds when the call is in a template that
// is instantiated after the builtin was first declared by an earlier use
// (Catch2 v2's REQUIRE expands to __builtin_constant_p(expr)).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
struct S {
  S() {}
  S(const S&) {}
  ~S() {}
};
inline int h(int c) { return __builtin_constant_p(c) ? 1 : 0; }
template <class T> int g(T) {
  S s;
  return __builtin_constant_p(s);
}
template <class T> int k(T t) { return __builtin_constant_p(t); }
int main() {
  if (g(1) != 0 || k(S()) != 0 || h(2) < 0) return 1;
  return 0;
}

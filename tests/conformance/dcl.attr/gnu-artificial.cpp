// GNU `artificial` (GNU extension; [dcl.attr.grammar]/5; spec/sema.md,
// clang ArtificialAttr): the attribute applies only to functions declared
// `inline`, also in its reserved `__artificial__` spelling beside
// `always_inline`; on a function that is only implicitly inline (in-class
// or `constexpr`) or on a variable it is ignored with a warning. The
// program runs unchanged.
//@ expect: run
//@ std: c++20
//@ strict-diags
//@ diag: 19:18 warning "'artificial' attribute only applies to inline functions"
//@ diag: 23:16 warning "'artificial' attribute only applies to inline functions"
//@ diag: 24:16 warning "'artificial' attribute only applies to inline functions"
//@ diag: 26:16 warning "'artificial' attribute only applies to inline functions"
//@ stdout: 12
extern "C" int printf(const char *, ...);
__attribute__((__always_inline__, __artificial__)) inline int a(int x) { return x + 1; }
struct S {
  __attribute__((artificial)) inline int m() const { return 2; }
  __attribute__((artificial)) int n() const { return 3; }
  template <class T>
  __attribute__((__always_inline__, __artificial__)) inline constexpr T t(T v) const { return v; }
};
__attribute__((artificial)) int b(int x) { return x; }
__attribute__((artificial)) constexpr int c(int x) { return x; }
[[gnu::artificial]] inline int d() { return 4; }
__attribute__((artificial)) int v;
int main() {
  S s;
  printf("%d\n", a(1) + s.m() + s.n() + s.t(1) + b(0) + c(0) + d() + v);
}

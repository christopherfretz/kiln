// [temp.dep.type]/6, [temp.over.link]/5: a `typename C<X>::T` inside a
// `decltype` operand written before the declarator-id of an out-of-line
// member names the member of the current instantiation, so
// `decltype(typename C<X>::T())` is equivalent to the in-class
// `decltype(T())` (clang's `RebuildExprInCurrentInstantiation`).
//@ expect: run
//@ strict-diags
//@ xfail(kiln): WP-1152 follow-up, decltype operands are not rebuilt in the current instantiation
//@ stdout: 7
extern "C" int printf(const char*, ...);
template <class X> struct C {
  using T = X;
  T v = 7;
  decltype(T()) d();
};
template <class X> decltype(typename C<X>::T()) C<X>::d() { return v; }
int main() {
  C<int> c;
  printf("%d\n", c.d());
}

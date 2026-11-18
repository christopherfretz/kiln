// [dcl.align]/2, [expr.prim.lambda.capture]/11, [expr.const]/10: an
// odr-use of a by-copy capture in an alignment argument inside a lambda
// body names the closure's member, which is no constant expression. kiln
// reads the captured variable only for the keyword `alignas` in a lambda
// outside templates, where clang 22 and GCC 16.1 both accept
// (closure-copy-odr-use-reads-variable-run.cpp); GNU `aligned(*&i)` and a
// lambda in a function template keep the text's reading, as GCC 16.1 does
// (clang accepts both).
//@ xfail(clang): clang 19 and 22 read the captured variable in every alignment argument
//@ expect: compile-fail
//@ diag: 23:35 error "expression is not an integral constant expression"
//@ diag: 16:13 error "expression is not an integral constant expression"
template <class Q> int t() {
  const Q j = 8;
  return [=] {
    alignas(*&j) char c;
    return (int)sizeof(c);
  }();
}
int f() {
  const int i = 16;
  return [=] {
    char d __attribute__((aligned(*&i)));
    return (int)sizeof(d);
  }();
}
int main() { return f() + t<int>(); }

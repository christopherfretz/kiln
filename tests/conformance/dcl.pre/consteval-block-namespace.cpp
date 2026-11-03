// [dcl.pre]/P3289R1: `consteval compound-statement` is a
// consteval-block-declaration, evaluated once where it is declared as if
// `static_assert((void)[] -> void consteval compound-statement (), true)`.
// Locals, static_assert, `return;` and calls to constexpr and consteval
// functions work; the dump shows a ConstevalBlockDecl holding the call of
// its consteval lambda (spec/ast-dump.md; kiln-only golden: clang 19 has
// no consteval blocks).
//@ expect: ast
//@ ast-filter: test_ns
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ strict-diags
int rt();
constexpr int sq(int x) { return x * x; }
consteval int cube(int x) { return x * x * x; }
namespace test_ns {
consteval {
  int a = sq(3);
  static_assert(cube(2) == 8);
  if (a != 9) rt();
  return;
}
}
// `consteval` followed by anything but `{` is the decl-specifier.
consteval int one() { return 1; }
static_assert(one() == 1);
consteval { if (one() != 1) rt(); }
consteval {}

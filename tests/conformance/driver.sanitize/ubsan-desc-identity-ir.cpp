// clang keys a UBSan type descriptor by the QualType object, sugar
// included: `int (&)[4]`'s ParenType-wrapped `int[4]` and the deduced
// `auto` `int` that `g(x) + g(x)` keeps as its common sugar each get their
// own descriptor though they print like an earlier one; kiln has no
// ParenType node and keys deduced types by their name. (`S` needs a second
// member: a trailing `a[4]` is a flexible array and is not checked.)
// spec/codegen.md "Sanitizers", docs/design/sanitizers.md §2, WP-871.
//@ expect: ir
//@ std: c++20
//@ args: -fsanitize=array-bounds,signed-integer-overflow -Xclang -disable-llvm-passes
//@ xfail(kiln): WP-871 follow-up, type descriptors are keyed by name, not by the sugared QualType (ParenType, deduced auto)
struct S { int a[4]; int n; };
int f1(S *s, int i) { return s->a[i]; }
int f2(int (&r)[4], int i) { return r[i]; }
auto g(int x) { return x; }
int f3(int x) { return 1 + x; }
int f4(int x) { return g(x) + g(x); }

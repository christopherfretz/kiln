//@ args: -momit-leaf-frame-pointer
// -momit-leaf-frame-pointer at -O0: x86-64's "all" becomes "non-leaf"
// (flag 1); AArch64 keeps its default "non-leaf" (spec/codegen.md
// "Frame pointer").
int callee(int);
int caller(int x) { return callee(x) + 1; }
int leaf(int x) { return x * 2; }
struct S {
  int v;
  S(int a) : v(a) {}
};
S global(3);

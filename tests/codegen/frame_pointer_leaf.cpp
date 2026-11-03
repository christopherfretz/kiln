//@ args: -mno-omit-leaf-frame-pointer
// -mno-omit-leaf-frame-pointer: leaf functions keep the frame pointer
// too, so AArch64's default "non-leaf" becomes "all" (flag 2); x86-64
// keeps "all" (spec/codegen.md "Frame pointer").
int callee(int);
int caller(int x) { return callee(x) + 1; }
int leaf(int x) { return x * 2; }
struct S {
  int v;
  S(int a) : v(a) {}
};
S global(3);

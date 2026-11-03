//@ args: -O1 -fno-omit-frame-pointer
// -fno-omit-frame-pointer above -O0: x86-64 keeps the frame pointer it
// would omit when optimizing ("all", flag 2), AArch64 its default
// "non-leaf" (flag 1) (spec/codegen.md "Frame pointer").
int callee(int);
int caller(int x) { return callee(x) + 1; }
int leaf(int x) { return x * 2; }
struct S {
  int v;
  S(int a) : v(a) {}
};
S global(3);

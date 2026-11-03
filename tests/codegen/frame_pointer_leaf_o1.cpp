//@ args: -O1 -momit-leaf-frame-pointer -mno-omit-leaf-frame-pointer
// The last -m[no-]omit-leaf-frame-pointer wins; above -O0 x86-64 omits
// the frame pointer anyway (no attribute, no flag), AArch64 keeps it in
// every function ("all") (spec/codegen.md "Frame pointer").
int callee(int);
int caller(int x) { return callee(x) + 1; }
int leaf(int x) { return x * 2; }
struct S {
  int v;
  S(int a) : v(a) {}
};
S global(3);

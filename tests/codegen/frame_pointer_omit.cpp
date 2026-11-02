//@ args: -fomit-frame-pointer
// clang's -fomit-frame-pointer: no function keeps a frame pointer on
// either target, so neither the "frame-pointer" attribute nor the
// frame-pointer module flag is emitted (spec/codegen.md "Frame pointer").
int callee(int);
int caller(int x) { return callee(x) + 1; }
int leaf(int x) { return x * 2; }
struct S {
  int v;
  S(int a) : v(a) {}
};
S global(3);

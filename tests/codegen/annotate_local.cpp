// clang's `annotate` attribute on local variables, parameters, static
// locals and lambdas (WP-1113, [dcl.attr.grammar]/5, [stmt.dcl]/2): an
// automatic variable or parameter gets one `@llvm.var.annotation` call per
// attribute on its storage once it is set up (after a parameter's store,
// before a variable's initializer), a static local an
// `@llvm.global.annotations` element when its declaration is emitted, and
// a lambda's call operator one at the end of the module.
int f(int x [[clang::annotate("param", 2)]]) {
  int y [[clang::annotate("local", 7)]] = x;
  int z [[clang::annotate("local")]] [[clang::annotate("second", 7)]] = x;
  int u [[clang::annotate("uninit")]];
  u = y;
  static int s [[clang::annotate("static")]] = 3;
  return y + z + s + u;
}
struct Big { int a[8]; };
int byref(Big b [[clang::annotate("big")]]) { return b.a[1]; }
void lam() {
  auto l = [](int q) {
    int lv [[clang::annotate("lamvar")]] = q;
    static int ls [[clang::annotate("lamstatic")]];
    return lv + ls;
  };
  l(1);
  auto g = [] [[clang::annotate("lamfn")]] () {};
  g();
}
int dead() {
  return 1;
  int after [[clang::annotate("dead")]] = 2;
}

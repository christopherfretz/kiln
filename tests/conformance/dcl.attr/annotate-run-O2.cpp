// clang's `annotate` lowered (WP-1113): `llvm.global.annotations` on
// functions and variables, `llvm.var.annotation` on locals and parameters,
// `llvm.ptr.annotation` on every annotated field access. The optimizer
// drops the intrinsics and passes the pointer through, so annotated code
// runs as without them. [dcl.attr.grammar]/5, [basic.start.main]/5.
//@ expect: run
//@ args: -O2
//@ exit: 33
struct S {
  int a [[clang::annotate("fa")]];
  int &r [[clang::annotate("fr", 2)]];
};
union U { int i [[clang::annotate("ui")]]; float f; };
int g [[clang::annotate("g", 1, 2.5, "s")]] = 5;
template <class T> T tv [[clang::annotate("tv")]] = T(3);
[[clang::annotate("fn")]] int twice(int x [[clang::annotate("p")]]) {
  int l [[clang::annotate("l")]] = x;
  static int calls [[clang::annotate("st")]];
  ++calls;
  return l * 2 + calls - 1;
}
int main() {
  int z = 10;
  S s{4, z};
  s.a += s.r;
  U u;
  u.i = 7;
  return twice(s.a) + g + tv<int> + u.i - 1 - z + 1;
}

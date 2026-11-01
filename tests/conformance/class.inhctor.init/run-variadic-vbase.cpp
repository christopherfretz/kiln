// [class.inhctor.init]/1 with a C-variadic inherited constructor: the
// variadic arguments cannot be forwarded, so the inheriting constructor is
// inlined at each call site (clang `EmitInlinedInheritingCXXConstructorCall`).
// When the inheriting class has virtual bases, its base-object variant
// takes the VTT of the enclosing subobject as an implicit parameter
// ([class.base.init]/7: only the most derived class constructs V). clang 19
// crashes in `GetVTTParameter` for every such call (it computes the VTT
// after entering the inlined scope), so the expected output is hand-checked.
//@ expect: run
//@ oracle: none
//@ skip(clang): clang 19 crashes inlining a C-variadic inheriting constructor of a base with virtual bases
//@ stdout: V 1
//@ stdout: B 3
//@ stdout: V 1
//@ stdout: B 6
//@ stdout: V 1
//@ stdout: B 2
//@ stdout: V 1
//@ stdout: B 7
//@ stdout: 3 5 9 1 6 1 2 4 1 7 1
extern "C" int printf(const char*, ...);
struct V { V() : v(1) { printf("V %d\n", v); } int v; };
struct B { B(int n, ...) : s(n) { printf("B %d\n", n); } int s; };
struct D : B, virtual V { using B::B; int x = 5; };
struct E : D { E() : D(3, 1, 2) {} int y = 9; };
struct H : virtual V, D { H(int a) : D(a, 2.5) {} };
struct F : D { using D::D; int z = 4; };
int main() {
  E e;
  H h(6);
  F f(2, 10, 20);
  D d(7);
  printf("%d %d %d %d %d %d %d %d %d %d %d\n", e.s, e.x, e.y, e.v, h.s, h.v, f.s, f.z,
         f.v, d.s, d.v);
  return 0;
}

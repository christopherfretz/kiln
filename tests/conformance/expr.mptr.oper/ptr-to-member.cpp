// [expr.mptr.oper]: .* and ->* bind a pointer to member to an object of the
// member's class or of a class derived from it.
//@ expect: run
//@ stdout: 1 2 12 22 5
//@ xfail(kiln): needs WP-065 parser classes
extern "C" int printf(const char*, ...);
struct B { int a = 1, b = 2; int add(int k) const { return a + k; } };
struct D : B { int c = 5; };
int main() {
  D d;
  D *pd = &d;
  int B::*pa = &B::a;
  int B::*pb = &B::b;
  int (B::*pf)(int) const = &B::add;
  int D::*pc = &D::c;
  printf("%d %d %d %d %d\n", d.*pa, pd->*pb, (d.*pf)(11), (pd->*pf)(21), d.*pc);
}

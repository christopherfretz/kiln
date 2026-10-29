// [class.derived], [conv.ptr]/3, [dcl.init.ref]: a derived object contains
// its base subobjects; pointers and references to it convert to its public
// bases; members of the bases are members of the derived class.
//@ expect: run
//@ stdout: 1 2 3 1 2 6
//@ xfail(kiln): needs WP-065 parser classes
extern "C" int printf(const char*, ...);
struct A { int a = 1; int geta() const { return a; } };
struct B { int b = 2; };
struct C : A, B { int c = 3; int sum() const { return a + b + c; } };
int main() {
  C c;
  A *pa = &c;
  B &rb = c;
  printf("%d %d %d %d %d %d\n", c.a, c.b, c.c, pa->geta(), rb.b, c.sum());
}

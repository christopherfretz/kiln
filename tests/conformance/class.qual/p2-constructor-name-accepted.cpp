// [class.qual]/2: the class named through a derived class, an elaborated
// type specifier, a mem-initializer, a constructor definition and a nested
// name specifier still name the class.
//@ expect: compile-ok
//@ strict-diags
struct S { S(int) {} S(int, int); typedef int I; };
struct D : S { D(); };
S::S(int, int) {}
void f() {
  D::S a(1);
  struct S::S b = S(2);
  S::S::I i = 3;
  (void)a; (void)b; (void)i;
}
D::D() : S::S(7) {}

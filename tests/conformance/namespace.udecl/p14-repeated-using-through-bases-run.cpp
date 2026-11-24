// [namespace.udecl]/1, /14, [class.member.lookup]: two using-declarators
// of one class naming the same base member through two classes each
// introduce it (WP-3403); the member is one candidate, a conversion
// function one conversion, and a member function of the class declared
// before or after them hides it.
//@ expect: run
//@ stdout: 2 7 | 3 2 | 3 2
extern "C" int printf(const char *, ...);
struct A {
  int f() { return 1; }
  int f(int) { return 2; }
  operator int() const { return 7; }
};
struct B1 : virtual A {};
struct B2 : virtual A {};
struct D : B1, B2 {
  using B1::f;
  using B2::f;
  using B1::operator int;
  using B2::operator int;
};
struct E : B1, B2 {
  using B1::f;
  using B2::f;
  int f() { return 3; }
};
struct G : B1, B2 {
  int f() { return 3; }
  using B1::f;
  using B2::f;
};
int main() {
  D d;
  E e;
  G g;
  int i = d;
  printf("%d %d | ", d.f(0), i);
  printf("%d %d | ", e.f(), e.f(0));
  printf("%d %d\n", g.f(), g.f(0));
  return 0;
}

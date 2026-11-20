// [dcl.attr.nouniqueaddr] (WP-1739): the values of constant-initialized
// objects with potentially-overlapping members whose tail padding a
// later member reuses (overlapping-member-constant-ir.cpp), including a
// member with a virtual base, whose constant (P3533) carries the
// virtual base's value.
//@ expect: run
//@ stdout: 1 2 3 | 7 8 9 10 | 1 2 3 | 4 5 6 | 6 5 7
extern "C" int printf(const char *, ...);
struct A {
  int x;
  char c;
  constexpr A(int a, char b) : x(a), c(b) {}
};
struct B { [[no_unique_address]] A a; char d; };
B b{{1, 2}, 3};
struct Base { char z; [[no_unique_address]] A a; };
struct Der : Base { char d; };
Der der{{7, {8, 9}}, 10};
union U {
  [[no_unique_address]] A a;
  int i;
  constexpr U() : a(1, 2) {}
};
struct E { [[no_unique_address]] U u; char d; };
E e{{}, 3};
struct B5 { [[no_unique_address]] A a; char e; [[no_unique_address]] A b; char f; };
B5 b5{{1, 2}, 3, {4, 5}, 6};
struct VB { int y = 5; };
struct W : virtual VB { char c = 6; };
struct G { [[no_unique_address]] W w; char d = 7; };
G g{};
int main() {
  printf("%d %d %d | %d %d %d %d | %d %d %d | %d %d %d | %d %d %d\n", b.a.x, b.a.c, b.d, der.z,
         der.a.x, der.a.c, der.d, e.u.a.x, e.u.a.c, e.d, b5.b.x, b5.b.c, b5.f, g.w.c, g.w.y, g.d);
}

// [class.member.lookup]/8, [expr.ref]/7: a member named `Q::m` is found
// in `Q`, and the object is converted to its `Q` subobject first, so a
// class with two `impl` subobjects (through `tramp<0>` and `tramp<1>`)
// reaches each one without ambiguity: data members, member functions
// (explicit and implicit object), and through a pointer.
//@ expect: run
//@ stdout: 2 3 2 3 2 3 5
#include <cstdio>
struct impl {
  int c;
  int color() const { return c; }
};
template <int> struct tramp : impl {};
struct n1 : tramp<0> {};
struct n2 : n1, tramp<1> {
  int f() const { return tramp<1>::color(); }
  int g() const { return n1::color(); }
  int h() { return tramp<1>::c + this->tramp<0>::c; }
};
int main() {
  n2 x;
  static_cast<tramp<1>&>(x).c = 2;
  static_cast<n1&>(x).c = 3;
  n2* p = &x;
  std::printf("%d %d %d %d %d %d %d\n", x.tramp<1>::c, x.n1::c, p->tramp<1>::color(),
              p->tramp<0>::color(), x.f(), x.g(), x.h());
}

// [expr.ref]/1, /4, [basic.lookup.qual.general]/2: a member access may
// name a qualified-id. The first component of the qualifier is looked up
// in the object's class and then in the context of the expression; the
// named member must be a member of the object's class or of a base. A
// qualified call does not dispatch virtually ([class.virtual]/16).
//@ expect: run
//@ stdout: 2 2 10 20 | 1 1 1 | 30 3 | 1 2 | 11 1 | 21 22
extern "C" int printf(const char*, ...);

namespace N {
struct B {
  int m = 10;
  virtual int f() const { return 1; }
  int g(int) const { return 1; }
  int g(double) const { return 2; }
};
}

struct D : N::B {
  int m = 20;
  int f() const override { return 2; }
  typedef N::B Base;
  int both() const { return this->Base::m + this->D::m; }
  int base_f() const { return this->N::B::f(); }
};

template <class T> struct TD : T {
  int m = 3;
  int via_base() const { return this->T::m + this->TD::m; }
  int call() const { return this->T::f(); }
};

template <class T> int dep(const T& t) { return t.Base::m + t.T::f(); }
template <class T> int dep_ns(const T* p) { return p->N::B::f() + p->Base::g(1.0); }

constexpr int cx() {
  struct A { int a = 1; constexpr int get() const { return a; } };
  struct C : A { int a = 2; constexpr int get() const { return a; } };
  C c;
  return c.A::get() * 10 + c.C::get() + c.A::a * 0;
}
static_assert(cx() == 12);

int main() {
  D d;
  N::B& b = d;
  printf("%d %d %d %d | ", b.f(), d.f(), d.N::B::m, d.D::m);
  printf("%d %d %d | ", b.N::B::f(), d.Base::f(), d.base_f());
  printf("%d %d | ", d.both(), TD<D>().via_base() - 20);
  printf("%d %d | ", d.Base::g(1), (&d)->::N::B::g(1.5));
  printf("%d %d | ", dep(d) - 1, TD<D>().call() - 1);
  printf("%d %d\n", dep_ns(&d) + 18, cx() + 10);
}

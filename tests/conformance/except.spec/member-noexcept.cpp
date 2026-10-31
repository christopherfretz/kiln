// [except.spec]/2,13 and [class.mem.general]/7: a member's
// noexcept-specifier is a complete-class context; it is evaluated when
// needed, and is part of the function type ([dcl.fct]/14, [conv.fctptr]).
// A dependent one in a class template is instantiated only when needed
// ([temp.inst]/14).
//@ expect: compile-ok
struct A {
  void f() noexcept(true);
  void g() noexcept(sizeof(A) > 100);
  void h() noexcept(noexcept(f()));
  void i() noexcept(noexcept(this->g()));
  static void s() noexcept(N > 0);
  int x;
  static constexpr int N = 1;
  A() noexcept(noexcept(x + 1));
  A(int) noexcept(false);
  void j() throw();
};
static_assert(noexcept(A().f()));
static_assert(!noexcept(A().g()));
static_assert(noexcept(A().h()));
static_assert(!noexcept(A().i()));
static_assert(noexcept(A::s()));
static_assert(noexcept(A()));
static_assert(!noexcept(A(1)));
static_assert(noexcept(A().j()));
void (*p)() noexcept = &A::s;
void (A::*pm)() noexcept = &A::f;
void (A::*pm2)() = &A::g;
template <class T> struct B {
  void f() noexcept(noexcept(T())) {}
  void g() noexcept(sizeof(T) > 1);
  void h() noexcept(noexcept(f()));
};
struct Th { Th(); };
static_assert(noexcept(B<int>().f()));
static_assert(!noexcept(B<Th>().f()));
static_assert(noexcept(B<long>().g()));
static_assert(!noexcept(B<char>().g()));
static_assert(!noexcept(B<Th>().h()));
static_assert(noexcept(B<int>().h()));
struct C {
  template <class T> void f(T) noexcept(sizeof(T) > 1);
};
static_assert(noexcept(C().f(1L)));
static_assert(!noexcept(C().f('c')));
void A::f() noexcept {}
template <class T> struct L {
  void f() noexcept(T::value);
  void g() noexcept(noexcept(T::h()));
  void unused() noexcept(T::no_such_member);
};
struct Y { static constexpr bool value = true; static void h(); };
L<Y> l;
static_assert(noexcept(l.f()));
static_assert(!noexcept(l.g()));
template <class T> struct Cp { Cp(const Cp&) noexcept(sizeof(T) < 4); Cp(); };
struct Hc { Cp<char> c; };
struct Ic { Cp<double> c; };
extern const Hc hc;
extern const Ic ic;
static_assert(noexcept(Hc(hc)));
static_assert(!noexcept(Ic(ic)));
template <class T> struct Pr {
  void f(T t) noexcept(noexcept(t.h()));
  void g() noexcept(noexcept(this->f(T())));
};
Pr<Y> pr;
static_assert(!noexcept(pr.f(Y())));
static_assert(!noexcept(pr.g()));
template <class T> struct Q {
  static void s() noexcept(T::other);
  void m() noexcept(T::other);
};
struct Yes { static constexpr bool other = true; };
struct No { static constexpr bool other = false; };
void (*qs)() noexcept = &Q<Yes>::s;
void (*qs2)() = &Q<No>::s;
void (*qs3)() noexcept = Q<Yes>::s;
void (Q<Yes>::*qm)() noexcept = &Q<Yes>::m;
void (Q<No>::*qm2)() = &Q<No>::m;

// [conv.mem]/2 with [expr.cast]/4: a C-style cast converts a pointer to
// member through an inaccessible base, reinterpret_cast does no base check,
// and where the base is accessible (a member, a friend, a derived class's
// member for a protected base) the implicit conversion is valid; in a
// SFINAE context an inaccessible base is a substitution failure.
// Found running GCC's C++ testsuite.
//@ expect: compile-ok
//@ strict-diags
struct B { int m; int f() const; };
struct D : private B {
  static int D::*inside() { return &B::m; }
  friend int D::*fr();
};
int D::*fr() { return &B::m; }
struct P : protected B {
  void in() { int P::*q = &B::m; (void)q; }
};
int D::*c1 = (int D::*)&B::m;
int (D::*c2)() const = (int (D::*)() const)&B::f;
int B::*c3 = (int B::*)c1;
int B::*r = reinterpret_cast<int B::*>(c1);
template <class U> auto sf(int) -> decltype((void)(int U::*)nullptr, (int U::*)nullptr == &B::m, char());
template <class U> long sf(...);
static_assert(sizeof(sf<D>(0)) == sizeof(long), "");
struct Q : B {};
static_assert(sizeof(sf<Q>(0)) == sizeof(char), "");

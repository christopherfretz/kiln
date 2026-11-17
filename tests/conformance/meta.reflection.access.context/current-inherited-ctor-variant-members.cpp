// [meta.reflection.access.context]/5 (P2996R13): access_context::current()
// in a default member initializer used by an inherited constructor
// reports the inherited (base) constructor's scope, as it does for the
// base's own; in a trailing requires-clause of a member function of a
// class template specialization, the class. [meta.reflection.access.queries]
// /4: a member of an anonymous union is accessible as the anonymous
// union's member of the enclosing class (a possibly indirect variant
// member). [class.protected]/1 adds no check to a reflect-expression
// naming a protected member: `^^A::f` names no object. Found running
// GCC's C++ testsuite.
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ xfail(gcc): GCC 16.1 finds a member of a nested anonymous union inaccessible in its own class, and applies [class.protected] to ^^A::m
//@ expect: compile-ok
//@ strict-diags
#include <meta>
using namespace std::meta;

struct B {
  consteval B(int) {}
  consteval B(int, int) {}
  info s = access_context::current().scope();
};
struct C : B { using B::B; };
struct D : C { using C::C; };
static_assert(is_constructor(B{1}.s) && parent_of(B{1}.s) == ^^B);
static_assert(is_constructor(C{1, 2}.s) && parent_of(C{1, 2}.s) == ^^B);
static_assert(is_constructor(D{1}.s) && parent_of(D{1}.s) == ^^B);

template <auto R> struct T {
  consteval bool fn()
    requires(is_type(access_context::current().scope()))
  {
    return access_context::current().scope() == ^^fn;
  }
};
static_assert(T<0>{}.fn());

struct N {
  union { int a; };
protected:
  union { int b; };
private:
  union { union { int c; }; };
public:
  static constexpr auto ctx = access_context::current();
  static constexpr info ra = ^^a, rb = ^^b, rc = ^^c;
};
struct O : N { static constexpr auto ctx = access_context::current(); };
constexpr auto none = access_context::unprivileged();
static_assert(is_accessible(N::ra, none) && is_accessible(N::ra, none.via(^^N)));
static_assert(is_accessible(N::rb, N::ctx) && is_accessible(N::rc, N::ctx));
static_assert(!is_accessible(N::rb, none.via(^^N)) && !is_accessible(N::rc, none.via(^^N)));
static_assert(is_accessible(N::rb, O::ctx.via(^^O)) && !is_accessible(N::rc, O::ctx.via(^^O)));
static_assert(!is_accessible(N::rb, none.via(^^O)));

class A {
protected:
  int m;
  void f();
  static void g();
};
struct DA : A {
  static constexpr info rf = ^^A::f, rm = ^^A::m, rg = ^^A::g;
};
static_assert(DA::rf == members_of(^^A, access_context::unchecked())[1]);

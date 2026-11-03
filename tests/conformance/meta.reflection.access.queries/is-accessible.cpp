// [meta.reflection.access.queries] (P2996R13): is_accessible(r, ctx) is
// [class.access]'s answer for naming the member or base r from ctx's
// scope, through ctx's designating class (default: the member's class);
// the null scope (unchecked()) accepts everything; an entity that is not
// a class member or base is always accessible; a designating class that
// is not the member's class nor derived from it accepts nothing. A
// protected member named from a derived class's member needs that class
// as the designating class ([class.protected]; GCC 16.1 agrees). The
// first block is the IS example.
//@ args: -freflection -I tests/support
//@ skip(clang): clang does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
//@ oracle: gcc
#ifdef __KILN__
#include "kiln_meta.h"
#else
#include <meta>
#endif
using namespace std::meta;

consteval access_context fn() {
  return access_context::current();
}

class Cls {
    int mem;
    friend consteval access_context fn();
public:
    static constexpr auto r = ^^mem;
};

static_assert(is_accessible(Cls::r, fn()));                             // OK
static_assert(!is_accessible(Cls::r, access_context::current()));      // OK
static_assert(is_accessible(Cls::r, access_context::unchecked()));     // OK

// Public, protected, private from outside, a member and a friend.
class A {
  int priv;

 protected:
  int prot;

 public:
  int pub;
  static constexpr info rpriv = ^^priv, rprot = ^^prot;
  static consteval bool inside() {
    return is_accessible(rpriv, access_context::current()) &&
           is_accessible(rprot, access_context::current());
  }
};
constexpr auto here = access_context::current();
static_assert(is_accessible(^^A::pub, here));
static_assert(!is_accessible(A::rpriv, here));
static_assert(!is_accessible(A::rprot, here));
static_assert(!is_accessible(A::rpriv, access_context::unprivileged()));
static_assert(is_accessible(^^A::pub, access_context::unprivileged()));
static_assert(A::inside());

// [class.protected]: from a member of D, prot is accessible through D.
struct D : A {
  static consteval bool own() { return is_accessible(rprot, access_context::current().via(^^D)); }
  static consteval bool through_a() { return is_accessible(rprot, access_context::current()); }
  static consteval bool priv() { return is_accessible(rpriv, access_context::current().via(^^D)); }
};
static_assert(D::own());
static_assert(!D::through_a());
static_assert(!D::priv());

// A designating class not derived from the member's class.
struct Other {
  int x;
};
static_assert(!is_accessible(^^A::pub, here.via(^^Other)));
static_assert(is_accessible(^^A::pub, here.via(^^D)));

// Bases: a private base is accessible from the derived class's members.
struct B {};
class E : B {
 public:
  static consteval bool inside() { return is_accessible(bases_of(^^E, access_context::unchecked())[0], access_context::current()); }
};
static_assert(!is_accessible(bases_of(^^E, access_context::unchecked())[0], here));
static_assert(E::inside());
struct F : B {};
static_assert(is_accessible(bases_of(^^F, access_context::unchecked())[0], here));

// Not class members: always accessible.
int global;
namespace N {}
static_assert(is_accessible(^^global, here));
static_assert(is_accessible(^^N, access_context::unprivileged()));
static_assert(is_accessible(^^int, here));

// Members of a nested class, from the enclosing class ([class.access.nest]).
class Outer {
  class In {
   public:
    int y;
  };

 public:
  static consteval bool f() { return is_accessible(^^In::y, access_context::current()); }
  static constexpr info rin = ^^In;
};
static_assert(Outer::f());
static_assert(!is_accessible(Outer::rin, here));

int main() {}

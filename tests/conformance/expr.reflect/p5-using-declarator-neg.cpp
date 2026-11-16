//@ expect: compile-fail
//@ args: -freflection
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 35:32 error "cannot take the reflection of 'fn', which names a declaration introduced by a using-declaration"
//@ diag: 36:32 error "cannot take the reflection of 'tfn', which names a declaration introduced by a using-declaration"
//@ diag: 37:28 error "cannot take the reflection of 'm', which names a declaration introduced by a using-declaration"
//@ diag: 40:11 error "cannot take the reflection of 'fn', which names a declaration introduced by a using-declaration"
// [expr.reflect]/5: a reflect-expression whose lookup finds a declaration
// that replaced a using-declarator is ill-formed (a using-declaration is
// not an entity). The accepted side: the target named directly, a name
// found through a using-directive, and a member found through a base
// rather than through the using-declaration. Found running GCC's C++
// testsuite.

namespace inner {
consteval int fn() { return 42; }
template <auto V> consteval int tfn() { return V; }
}
namespace outer {
using inner::fn;
using inner::tfn;
}
namespace via_directive {
using namespace inner;
}
struct A { int m; };
struct B : A { using A::m; };
struct C : A {};

constexpr auto ok1 = ^^inner::fn;
constexpr auto ok2 = ^^via_directive::fn;
constexpr auto ok3 = ^^C::m;
constexpr auto bad1 = ^^outer::fn;
constexpr auto bad2 = ^^outer::tfn;
constexpr auto bad3 = ^^B::m;
consteval void f() {
  using inner::fn;
  (void)^^fn;
}

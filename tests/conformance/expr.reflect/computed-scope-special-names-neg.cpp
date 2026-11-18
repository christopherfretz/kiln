// [expr.reflect]/7, [class.dtor]/1, [basic.lookup.qual]: the names after a
// computed scope (`decltype(e)::`) are looked up in the class it denotes,
// as after a class name: a missing operator or conversion function is an
// error, and the type-name after `~` must name that class. Found running
// GCC's C++ testsuite; GCC 16.1 rejects the same lines.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 17:36 error "no member named 'operator+' in '(lambda at "
//@ diag: 18:37 error "use of undeclared identifier 'T'"
//@ diag: 19:36 error "no member named 'operator int' in '(lambda at "
#include <meta>
auto a = [] { return 1; };
struct S { ~S(); } s;
constexpr auto e1 = ^^decltype(a)::operator+;
constexpr auto e2 = ^^decltype(s)::~T;
constexpr auto e3 = ^^decltype(a)::operator int;

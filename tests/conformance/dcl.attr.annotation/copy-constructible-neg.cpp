// [dcl.attr.annotation]/2, [meta.reflection.result]: the underlying
// constant is `std::meta::reflect_constant(e)`, which mandates
// `is_copy_constructible_v<T>`, also for a prvalue operand that would
// need no copy. A class whose copy constructor is usable but whose move
// constructor is deleted is fine. Found running GCC's C++ testsuite;
// GCC 16.1 agrees.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 22 does not implement P3394 annotations
//@ oracle: gcc
//@ strict-diags
//@ diag: 20:10 error "annotation has type 'U', which is not copy constructible"
//@ diag: 21:10 error "annotation has type 'U', which is not copy constructible"
#include <meta>

struct U { constexpr U() = default; U(const U&) = delete; };
struct V { int v; };
struct M { constexpr M() = default; constexpr M(const M&) = default; M(M&&) = delete; };
constexpr U u;
struct [[=U{}]] A {};
struct [[=u]] B {};
struct [[=V{1}]] C {};
struct [[=M{}]] D {};
static_assert(annotations_of(^^C).size() == 1);
static_assert(annotations_of(^^D).size() == 1);

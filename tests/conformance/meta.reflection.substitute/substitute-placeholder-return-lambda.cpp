// [meta.reflection.substitute]/2, [dcl.spec.auto.general]/12 (P2996R13):
// substitute of a generic lambda's call operator template deduces the
// specialization's return type by instantiating it, so the result can be
// spliced, called and extracted (found running GCC's C++ testsuite; GCC
// 16.1 agrees for a lambda and a static member template; WP-1674).
//@ expect: compile-ok
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ strict-diags
#include <meta>
using namespace std::meta;
using size = decltype(sizeof 0);

consteval info sizer(info ty) {
  static constexpr auto lam = []<class T>() static { return sizeof(T); };
  return substitute(^^decltype(lam)::template operator(), {ty});
}
constexpr info s = sizer(^^long);
static_assert([:s:]() == sizeof(long));
static_assert(is_static_member(s));
constexpr size (*p)() = extract<size (*)()>(s);
static_assert(p() == sizeof(long));

struct M { template<class T> static auto f() { return sizeof(T); } };
static_assert(can_substitute(^^M::f, {^^int}));
consteval bool in_fn() {
  static constexpr auto l = []<class T>() static { return T{}; };
  return can_substitute(^^decltype(l)::template operator(), {^^int});
}
static_assert(in_fn());

int main() {}

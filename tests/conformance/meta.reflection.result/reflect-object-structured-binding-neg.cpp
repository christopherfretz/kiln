// [meta.reflection.result] reflect_object (P2996R13) throws unless its
// argument is suitable as a constant template argument of reference type
// ([temp.arg.nontype]/3): a binding of a decomposed temporary (e is a
// reference bound to it, lifetime-extended) names a subobject of a
// temporary object; and a binding of an automatic decomposition in a
// consteval function names a subobject of an automatic variable, which
// a template argument cannot designate (the note names e as clang's does,
// '[lx, ly]'). The twin of reflect-object-structured-binding.cpp
// (WP-1783). GCC 16.1 rejects every line.
//@ expect: compile-fail
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ strict-diags
//@ diag: 38:16 error "constexpr variable 'n1' must be initialized by a constant expression"
//@ diag: 38:21 note "the object refers to a temporary object, which is not a valid constant template argument"
//@ diag: 38:21 error "call to consteval function 'std::meta::reflect_object<int>' is not a constant expression"
//@ diag: 38:21 note "the object refers to a temporary object, which is not a valid constant template argument"
//@ diag: 39:16 error "constexpr variable 'n2' must be initialized by a constant expression"
//@ diag: 39:21 note "the object refers to a temporary object, which is not a valid constant template argument"
//@ diag: 39:21 error "call to consteval function 'std::meta::reflect_object<const int>' is not a constant expression"
//@ diag: 39:21 note "the object refers to a temporary object, which is not a valid constant template argument"
//@ diag: 45:16 error "constexpr variable 'n3' must be initialized by a constant expression"
//@ diag: 43:10 note "pointer to subobject of '[lx, ly]' is not a constant expression"
//@ diag: 45:21 note "in call to 'local()'"
//@ diag: 42:8 note "declared here"
//@ diag: 45:21 error "call to consteval function 'local' is not a constant expression"
//@ diag: 43:10 note "pointer to subobject of '[lx, ly]' is not a constant expression"
//@ diag: 45:21 note "in call to 'local()'"
//@ diag: 42:8 note "declared here"
#include <meta>
using namespace std::meta;

struct P { int x; int y; };
auto&& [p, q] = P{1, 2};
const auto& [cp1, cq1] = P{3, 4};

constexpr info n1 = reflect_object(p);
constexpr info n2 = reflect_object(cq1);
consteval bool local() {
  P l{1, 2};
  auto [lx, ly] = l;
  return reflect_object(ly) != info();
}
constexpr bool n3 = local();

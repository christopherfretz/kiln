// [expr.ref]/7, [temp.dep.expr] (P2996R13): a member splice in a
// function template designates the member its reflection represents in
// each instantiation, also when members share a name (here the
// placeholder `_`), and never by name lookup. Found running GCC's C++
// testsuite.
//@ xfail(kiln): WP-1613: substitution rebuilds a member access from a splice by name lookup (templ/subst/exprs.rs)
//@ args: -freflection --gcc-toolchain=%{gcc_toolchain}
//@ skip(clang): clang does not implement P2996 reflection
//@ oracle: gcc
//@ expect: run
#include <meta>
struct S { int _; long _; short _; };
constexpr auto uctx = std::meta::access_context::unchecked();
template <int N> int bar() {
  S s = {1, 2, 3};
  s.[:nonstatic_data_members_of(^^S, uctx)[N]:] += 20;
  return s.[:nonstatic_data_members_of(^^S, uctx)[0]:] * 100 +
         s.[:nonstatic_data_members_of(^^S, uctx)[2]:];
}
int main() { return bar<0>() == 2103 && bar<2>() == 123 ? 0 : 1; }

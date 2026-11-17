// [namespace.alias]/2, [temp.dep.namespace]/1 (P2996R13): the rejected
// twin of dependent-namespace-alias-uses.cpp: the splice-specifier of a
// dependent namespace alias that designates a class, or the global
// namespace, is ill-formed in the instantiation (an uninstantiated
// template is not diagnosed). GCC 16.1 accepts an alias of the global
// namespace (docs/status/deviations.md). WP-1613.
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
//@ skip(gcc): GCC 16.1 accepts a namespace alias of the global namespace
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:23 error "splice in a namespace position does not designate a namespace"
//@ diag: 29:9 note "in instantiation of function template specialization 'fn<^^S>' requested here"
//@ diag: 22:23 error "the splice-specifier of a namespace alias cannot designate the global namespace"
//@ diag: 30:9 note "in instantiation of function template specialization 'g<^^::>' requested here"
struct S { static constexpr int v = 1; };
template <decltype(^^int) R> int fn() {
  namespace Alias = [:R:];
  return Alias::v;
}
template <decltype(^^int) R> int g() {
  namespace Alias = [:R:];
  return 0;
}
template <decltype(^^int) R> int unused() {
  namespace Alias = [:R:];
  return 0;
}
int a = fn<^^S>();
int b = g<^^::>();

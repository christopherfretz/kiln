// [temp.res.general]/4.2: a conversion-type-id is a type-only context, so
// `operator T::type()` needs no `typename` (GCC 16.1 accepts it; clang 22
// does not apply the rule there). WP-1495.
//@ expect: compile-ok
//@ std: c++20
//@ strict-diags
//@ xfail(clang): clang 22 requires 'typename' in a conversion-type-id
template<class T> struct S { operator T::type() const { return 1; } };
struct M { typedef int type; };
int i = S<M>();

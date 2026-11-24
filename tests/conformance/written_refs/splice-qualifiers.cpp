// The written-reference table (spec/ast.md §18.4, §18.11.2): a
// splice-scope-specifier `[: r :]::` and a splice-specialization-specifier
// (`template [: r :]<A>::`, `typename [: r :]<A>::`) have no entry, and
// the component after them is qualified by the specifier, as after a
// `decltype(...)::` ([expr.prim.id.qual]/1, [temp.names]/3):
// `q=specifier:L:C` gives the `[:`, or the `template` before it.
//@ written-refs: splice-qualifiers.refs
//@ std: c++26
//@ args: -freflection
//@ skip(clang): clang does not implement P2996 reflection
struct S {
  static int m;
  using type = int;
};
template <class T> struct X {
  static int v;
  using type = T;
};
template <class T> int X<T>::v = 0;
int a = [:^^S:]::m;
[:^^S:]::type b = 0;
int c = template [:^^X:]<int>::v;
typename [:^^X:]<long>::type e = 0;

// The written-reference table (spec/ast.md §18.4, §18.11.2): a
// `decltype(...)::` nested-name-specifier has no entry (the names of its
// operand have theirs), and the component after it is qualified by the
// specifier: `q=specifier:L:C` gives the `decltype` keyword, so the
// written name's extent (`qualified_lo`) begins there, as clang's
// `getBeginLoc` of `decltype(s)::m` does ([dcl.type.decltype],
// [expr.prim.id.qual]). A later component links the one before it; a
// `decltype` spelled by a macro is the macro's token.
//@ written-refs: decltype-qualifiers.refs
//@ std: c++20
struct S {
  static int m;
  static int f() { return 1; }
  struct N {
    static int k;
    using type = int;
  };
  using type = long;
  template <class T> static int tm() { return sizeof(T); }
};
S s;
int y = decltype(s)::m;
int z = decltype(s)::N::k;
decltype(s)::type w = 0;
decltype(s)::N::type x = 0;
int (*pf)() = &decltype(s)::f;
int c = decltype(s)::f() + decltype(s)::template tm<char>();
#define DT decltype(s)
int d = DT::m;
template <class T> int g(T t) {
  typename decltype(t)::type v = 0;
  return decltype(t)::m + decltype(t)::N::k + v;
}
int u = g(s);

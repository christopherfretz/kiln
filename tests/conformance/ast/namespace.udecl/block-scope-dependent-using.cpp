//@ expect: ast
// [namespace.udecl]/1, [temp.res.general]/4: a block-scope using-declarator
// whose nested-name-specifier is dependent declares an unresolved using
// value (UnresolvedUsingValueDecl); a use of its name in the template is
// an UnresolvedLookupExpr of dependent type that finds it, without ADL.
namespace N { template<class T> struct E { enum { a, b }; }; }
template<class T> int f() {
  using N::E<T>::a;
  int x = a;
  return a + x;
}

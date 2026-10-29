// [dcl.mptr]/1 with [stmt.ambig]/1 and [dcl.ambig.res]/2: a statement
// `I (C::* name);` whose nested-name-specifier has template-id components
// (`X<T>::*`, `X<T>::Y::*`, `::X<T>::*`, `N::Z<T>::*`,
// `T::template W<int>::*`) is a declaration, as is the type-id in `sizeof`.
// [temp.names]/3: after a variable `x`, `<` is less-than, so `I (x<1>::y);`
// is an expression (a functional cast of `(x < 1) > ::y`); the declared
// names are usable afterwards.
//@ args: -Wno-parentheses
//@ strict-diags
//@ diag: 23:3 warning "expression result unused"
template <class T> struct X { struct Y {}; };
namespace N { template <class T> struct Z {}; }
typedef int I;
int x = 1, y = 2;
template <class T> void f() {
  I (X<T>::*p1);
  I (X<T>::Y::*p2);
  I (::X<T>::*p3);
  I (N::Z<T>::*p4);
  I (T::template W<int>::*p5);
  (void)p1, (void)p2, (void)p3, (void)p4, (void)p5;
  (void)sizeof(I (X<T>::*));
  I (x<1>::y);
}

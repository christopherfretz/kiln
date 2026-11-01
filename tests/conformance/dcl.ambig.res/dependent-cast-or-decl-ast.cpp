// [stmt.ambig] / [dcl.ambig.res] inside templates: a dependent type `U`
// disambiguates exactly as a non-dependent class type does; `U::type`
// without `typename` is an expression (a call), with it a type.
//@ expect: ast
int a;
template<class U> void d1() { U(e); }
template<class U> void d2() { U(*p)(int); }
template<class U> int d4() { return (U)a; }
template<class U> int d5() { return (U(a)); }
template<class U> int d6() { return U(a) + 1; }
template<class U> void d7() { typename U::type(e); }
template<class U> void d8() { U::type(a); }
template<class U> void d9() { typename U::type(*q)(int); }
template<class U> void d10() { U(f), g = 1; }
template<class U> void d11() { U(h)[2]; }
template<class U> void d12() { for (U(i); ;) break; }
template<class U> void d13() { U(a)->m; }

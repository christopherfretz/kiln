// [temp.res.general]/4, [temp.names]/4: a dependent qualified name used
// as a type needs `typename` (outside the contexts where it is implied),
// and a dependent member template name after `::` needs `template`.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 13:43 error "use 'template' keyword to treat 'M' as a dependent template name"
//@ diag: 14:59 error "use 'template' keyword to treat 'Y' as a dependent template name"
//@ diag: 15:61 error "use of undeclared identifier 'p'"
//@ diag: 17:31 error "missing 'typename' prior to dependent type name 'T::nested::type'"
// Without `typename`, `T::type::template Z<int>::q * p` is a
// multiplication; `T::type y` in a member declaration and the alias and
// parameter forms are implicit typename contexts ([temp.res.general]/4).
template <class T> struct A { typename T::M<int>::type x; };
template <class T> using B = typename T::template X<int>::Y<long>::type;
template <class T> void f() { T::type::template Z<int>::q * p; }
template <class T> struct C { T::type y; };
template <class T> void g() { T::nested::type z; }
template <class T> struct D { using u = T::template M<int>; };
template <class T> int h(typename T::template M<int>::type);

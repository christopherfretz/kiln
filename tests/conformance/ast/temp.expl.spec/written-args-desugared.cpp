//@ expect: ast
//@ ast-filter: v
// [temp.expl.spec], [temp.explicit] (WP-1816): a class template specialization declared
// explicitly (an explicit specialization or an explicit instantiation) prints the template
// arguments written on that declaration wherever its type prints without sugar, so the
// desugared half of a type names it as written (`'A<N::X>':'A<X>'`), and a use spelled the
// same way needs no desugared half; an implicit specialization prints its converted arguments.
namespace N { struct X {}; }
using N::X;
template<class T> struct A { T t; };
template<> struct A<X> { int i; };
template<class T> struct B {};
template struct B<X>;
template<class T> struct C {};
A<X> v1;
A<N::X> v2;
B<X> v3;
B<N::X> v4;
C<X> v5;

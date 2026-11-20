// [temp.explicit]/3: `template struct N::S;` names a class by an
// elaborated-type-specifier; it must be a member class of a class template
// specialization (clang looks the name up as a reference). A second
// `template` starts no declaration ([temp.explicit]/1). WP-1740.
//
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:1 error "explicit instantiation of non-templated type 'S'"
//@ diag: 22:8 note "non-templated declaration is here"
//@ diag: 30:1 error "explicit instantiation of non-templated type 'N::C'"
//@ diag: 23:22 note "non-templated declaration is here"
//@ diag: 31:1 error "explicit instantiation of non-templated type 'I'"
//@ diag: 24:8 note "non-templated declaration is here"
//@ diag: 32:1 error "explicit instantiation of non-templated type 'W::X'"
//@ diag: 25:19 note "non-templated declaration is here"
//@ diag: 33:1 error "explicit instantiation of non-templated type 'U'"
//@ diag: 33:17 note "non-templated declaration is here"
//@ diag: 34:23 error "template 'A' cannot be referenced with the 'class' specifier"
//@ diag: 26:26 note "declared here"
//@ diag: 35:24 error "explicit instantiation of 'p' does not refer to a function template, variable template, member function, member class, or static data member"
//@ diag: 36:10 error "expected unqualified-id"
struct S {};
namespace N { struct C {}; }
struct I;
struct W { struct X {}; };
template<class T> struct A { struct B {}; };
template<class T> void f(T) {}

template struct S;
template struct N::C;
template struct I;
template struct W::X;
template struct U;
extern template class A;
template class A<int> *p;
template template<class T> void f(T);

// Well-formed: a member class of a class template specialization.
template struct A<int>::B;
extern template struct A<long>::B;

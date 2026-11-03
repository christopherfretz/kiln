// [temp.inst]/2, [basic.lookup.qual]/1: a nested-name-specifier naming a
// declared but undefined class template specialization is an incomplete
// class. clang 19 reports it at the template-name and twice in a
// decl-specifier at namespace or class scope.
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 31:4 error "implicit instantiation of undefined template 'N::X<int>'" dup(clang)
//@ diag: 30:38 note "template is declared here"
//@ diag: 32:6 error "implicit instantiation of undefined template 'N::X<int>'" dup(clang)
//@ diag: 30:38 note "template is declared here"
//@ diag: 33:12 error "implicit instantiation of undefined template 'N::A<int>::B<int>'" dup(clang)
//@ diag: 30:97 note "template is declared here"
//@ diag: 34:15 error "implicit instantiation of undefined template 'N::X<int>'"
//@ diag: 30:38 note "template is declared here"
//@ diag: 34:33 error "implicit instantiation of undefined template 'N::X<char>'"
//@ diag: 30:38 note "template is declared here"
//@ diag: 37:4 error "implicit instantiation of undefined template 'A<long>::B<int>'" dup(clang)
//@ diag: 35:57 note "template is declared here"
//@ diag: 38:16 error "implicit instantiation of undefined template 'N::X<int>'"
//@ diag: 30:38 note "template is declared here"
//@ diag: 39:15 error "implicit instantiation of undefined template 'N::X<char>'" dup(clang)
//@ diag: 30:38 note "template is declared here"
//@ diag: 40:10 error "implicit instantiation of undefined template 'N::X<long>'" dup(clang)
//@ diag: 30:38 note "template is declared here"
//@ diag: 41:12 error "implicit instantiation of undefined template 'N::X<short>'"
//@ diag: 30:38 note "template is declared here"
//@ diag: 42:4 error "implicit instantiation of undefined template 'N::X<unsigned int>'" dup(clang)
//@ diag: 30:38 note "template is declared here"
namespace N { template<class> struct X; template <class T> struct A { template <class U> struct B; }; }
N::X<int>::type t1;
::N::X<int>::type t2;
N::A<int>::B<int>::type t3;
void f() { N::X<int>::g(); ::N::X<char>::g(); }
template <class T> struct A { template <class U> struct B; };
using L = A<long>;
L::B<int>::type t4;
void f2() { N::X<int>::type t; }
struct M { N::X<char>::type m; };
const N::X<long>::type c = 0;
void g2(N::X<short>::type p);
N::X<unsigned>::type *q;

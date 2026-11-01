// [temp.inst]/3, [class.base.init]/9.1: an ill-formed default member
// initializer of a class template specialization is diagnosed where a
// constructor, an aggregate initialization or an exception specification
// first uses it, with the "in instantiation of default member initializer"
// note; once invalid, later uses are quiet. WP-223.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 17:45 note "in instantiation of default member initializer 'S<int>::k' requested here"
//@ diag: 22:8 note "in instantiation of member function 'S<int>::S' requested here"
//@ diag: 18:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 24:11 note "in instantiation of default member initializer 'A<int>::k' requested here"
//@ diag: 20:51 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 20:57 note "in instantiation of default member initializer 'O<int>::In::k' requested here"
//@ diag: 26:12 note "in instantiation of member function 'O<int>::In::In' requested here"

template <class T> struct S { int k = T::v; S() {} S(int x) : k(x) {} };
template <class T> struct A { int k = T::v; int j; };
template <class T> struct N { int k = T::v; };
template <class T> struct O { struct In { int k = T::v; In() {} }; };
S<int> s1(1);
S<int> s2;
S<int> s3;
A<int> a1{};
A<int> a2{1, 2};
O<int>::In i1;
N<int> n1(N<int>{3});

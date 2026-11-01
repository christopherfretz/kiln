// [temp.inst]: a class template specialization's arguments are canonical, so a
// member instantiated from it sees 'unsigned long', not the alias 'sz' the
// specialization was first named with (clang substitutes canonical arguments).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:51 error "type 'unsigned long' cannot be used prior to '::' because it has no members"
//@ diag: 17:9 note "in instantiation of template class 'A<unsigned long>' requested here"
//@ diag: 16:44 error "static assertion failed due to requirement 'sizeof(unsigned long) == 1': no"
//@ diag: 19:7 note "in instantiation of template class 'D<unsigned long>' requested here"
//@ diag: 16:54 note "expression evaluates to '8 == 1'"
//@ diag: 15:47 error "member reference base type 'unsigned long' is not a structure or union"
//@ diag: 18:23 note "in instantiation of member function 'C<unsigned long>::f' requested here"
using sz = unsigned long;
template<class T> struct A { static const int v = T::value; };
template<class T> struct C { void f() { T t; t.g(); } };
template<class T> struct D { static_assert(sizeof(T) == 1, "no"); };
int a = A<sz>::v;
void h() { C<sz> c; c.f(); }
D<sz> d;

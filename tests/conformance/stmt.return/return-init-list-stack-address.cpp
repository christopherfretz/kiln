// [dcl.init.list]/6, [stmt.return]: returning a braced list as a
// std::initializer_list returns the address of its local backing array
// (clang's -Wreturn-stack-address), also from an instantiation.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 10:41 warning "returning address of local temporary object"
//@ diag: 12:58 warning "returning address of local temporary object"
//@ diag: 13:10 note "in instantiation of function template specialization 't<long>' requested here"
#include <initializer_list>
std::initializer_list<int> f() { return {1, 2, 3}; }
std::initializer_list<int> ok(std::initializer_list<int> l) { return l; }
template <class T> std::initializer_list<T> t() { return {T(), T()}; }
auto u = t<long>();
struct S { int a; int b; };
S s() { return {1, 2}; }

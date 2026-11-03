// [temp.expl.spec]/1, [dcl.constexpr]/6, [dcl.init]/6: diagnostics name a
// variable template specialization as clang prints it (canonical arguments
// without trailing defaulted ones, even when written with a typedef), except
// "declaration of variable 'au' with deduced type 'auto' requires an
// initializer", which names the template.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 14:26 error "constexpr variable 'g<long>' must be initialized by a constant expression"
//@ diag: 16:26 error "constexpr variable 'g<long *>' must be initialized by a constant expression"
//@ diag: 18:17 error "declaration of reference variable 'rr<int>' requires an initializer"
//@ diag: 20:17 error "declaration of variable 'au' with deduced type 'auto' requires an initializer"
//@ diag: 22:26 error "constexpr variable 'dd<char>' must be initialized by a constant expression"
template<class T> constexpr int g = 1;
template<> constexpr int g<long>;
using L = long;
template<> constexpr int g<L *>;
template<class T> int &rr = T::x;
template<> int &rr<int>;
template<class T> auto au = 1;
template<> auto au<int>;
template<class T, class U = int> constexpr int dd = 1;
template<> constexpr int dd<char>;

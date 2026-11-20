// [class.base.init]/9, [dcl.init.ref]/1: a mem-initializer `r()` for a
// reference member value-initializes nothing; a reference must be bound,
// so it is ill-formed: "reference to type 'int' requires an initializer"
// at the parentheses, also when the member's type is dependent.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:40 error "reference to type 'int' requires an initializer"
//@ diag: 11:45 error "reference to type 'const int' requires an initializer"
//@ diag: 12:42 error "reference to type 'int' requires an initializer"
//@ diag: 13:8 note "in instantiation of member function 'U<int>::U' requested here"
struct S { int &r; const int &c; S() : r(), c() {} };
template<class T> struct U { T &r; U() : r() {} };
U<int> u;

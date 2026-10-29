// [class.static.data]/4, [temp.inst]/4: a constexpr static data member of
// dependent type without an initializer is diagnosed when its definition
// is instantiated (on reference), not with the class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 9:50 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 11:17 note "in instantiation of static data member 'D<int>::x' requested here"
//@ diag: 12:33 error "constexpr variable 'v' must be initialized by a constant expression"
template <class T> struct D { static constexpr T x; };
D<int> d;
int k = D<int>::x;
struct S { static constexpr int v; };

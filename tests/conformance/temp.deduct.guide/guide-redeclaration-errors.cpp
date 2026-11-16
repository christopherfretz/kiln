// [temp.deduct.guide]/3: two deduction guides of one class template shall
// not have equivalent parameter-declaration-clauses. Found running GCC's C++ testsuite.
//@ expect: compile-fail
//@ strict-diags
//@ args: -std=c++20
//@ diag: 17:1 error "redeclaration of deduction guide"
//@ diag: 16:1 note "previous declaration is here"
//@ diag: 19:1 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 17:1 note "previous declaration is here"
//@ diag: 21:10 error "redeclaration of deduction guide"
//@ diag: 20:1 note "previous declaration is here"
//@ diag: 24:20 error "redeclaration of deduction guide"
//@ diag: 22:20 note "previous declaration is here"
template <class> struct S {};
template <class> struct X {};
S() -> S<int>;
S() -> S<int>;
X() -> X<int>;
S() -> S<float>;
S(bool) -> S<int>;
explicit S(bool) -> S<int>;
template <class T> S(T, T) -> S<int>;
template <class T> X(T, T) -> X<int>;
template <class T> S(T, T) -> S<int>;
// Valid: different parameters, different return types of templates, and
// a constrained guide.
template <class T> S(T) -> S<typename T::foo>;
template <class T> S(T) -> S<typename T::bar>;
S(int) -> S<int>;
template <class T = int> S(int) -> S<char>;
template <class T> requires (sizeof(T) > 1) S(T *) -> S<T>;
template <class T> S(T *) -> S<T>;

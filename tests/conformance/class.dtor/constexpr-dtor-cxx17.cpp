// [class.dtor] (C++17; P0784R7 allows a constexpr destructor from C++20):
// a destructor cannot be declared `constexpr`. clang `err_constexpr_dtor`
// at the specifier: a definition, a declaration and its out-of-line
// definition, a defaulted one and a class template's (WP-1123).
//@ expect: compile-fail
//@ std: c++17
//@ strict-diags
//@ diag: 14:12 error "destructor cannot be declared constexpr"
//@ diag: 15:12 error "destructor cannot be declared constexpr"
//@ diag: 16:1 error "destructor cannot be declared constexpr"
//@ diag: 17:12 error "destructor cannot be declared constexpr"
//@ diag: 18:32 error "destructor cannot be declared constexpr"

struct A { constexpr ~A() {} };
struct B { constexpr ~B(); };
constexpr B::~B() {}
struct C { constexpr ~C() = default; };
template <class T> struct T1 { constexpr ~T1() {} };

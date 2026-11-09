// [dcl.constexpr]/3 (C++20, removed by P2448R2 in C++23): the literal-type
// rule for the return and parameter types of a consteval function or
// constructor and of a consteval lambda; from C++20 the note names a
// destructor that is not constexpr. clang `CheckConstexprReturnType` /
// `CheckConstexprParameterTypes` (WP-1123).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 23:14 error "consteval function's return type 'NL' is not a literal type"
//@ diag: 20:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 24:22 error "consteval function's 1st parameter type 'D' is not a literal type"
//@ diag: 21:29 note "'D' is not literal because its destructor is not constexpr"
//@ diag: 25:30 error "consteval constructor's 2nd parameter type 'D' is not a literal type"
//@ diag: 21:29 note "'D' is not literal because its destructor is not constexpr"
//@ diag: 26:13 error "constexpr function's return type 'D' is not a literal type"
//@ diag: 21:29 note "'D' is not literal because its destructor is not constexpr"
//@ diag: 27:20 error "consteval function's 1st parameter type 'NL' is not a literal type"
//@ diag: 20:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"

struct NL { NL(); };
struct D { constexpr D() {} ~D() {} };

consteval NL ret() { return NL(); }
consteval int param(D) { return 1; }
struct C { consteval C(int, D) {} };
constexpr D constexpr_ret() { return D(); }
auto lambda = [](NL) consteval { return 1; };

// Valid: a constexpr destructor makes the class literal from C++20.
struct E { constexpr E() {} constexpr ~E() {} };
consteval E ok(E e) { return e; }

// [dcl.constexpr]/3 (C++20): a constexpr function's body shall not define a
// variable of non-literal type before C++23; the error names why the type is
// not literal and makes the function invalid (clang CheckConstexprDeclStmt).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:23 error "variable of non-literal type 'A' cannot be defined in a constexpr function before C++23"
//@ diag: 13:8 note "struct with virtual base class is not a literal type"
//@ diag: 13:12 note "virtual base class declared here"
//@ diag: 16:23 error "variable of non-literal type 'B' cannot be defined in a constexpr function before C++23"
//@ diag: 14:8 note "'B' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
struct V {};
struct A : virtual V { A(); };
struct B { B(); ~B(); };
constexpr int f() { A x; return 0; }
constexpr int g() { B x; return 0; }

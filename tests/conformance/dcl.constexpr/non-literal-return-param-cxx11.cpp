// [dcl.constexpr]/3 (C++11): the return type (other than for a constructor)
// and each parameter type of a constexpr function are literal types; `void`
// is not a literal type before C++14 ([basic.types]/10). clang
// `CheckConstexprReturnType` / `CheckConstexprParameterTypes` at the
// definition (a dependent type waits for, and is then not checked at,
// instantiation), with the `RequireLiteralType` note: before C++20 a
// user-provided destructor (WP-1123).
//@ expect: compile-fail
//@ std: c++11
//@ strict-diags
//@ diag: 30:14 error "constexpr function's return type 'NL' is not a literal type"
//@ diag: 25:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 31:28 error "constexpr function's 2nd parameter type 'NL' is not a literal type"
//@ diag: 25:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 32:27 error "constexpr function's 1st parameter type 'D' is not a literal type"
//@ diag: 26:29 note "'D' is not literal because it has a user-provided destructor"
//@ diag: 33:16 error "constexpr function's return type 'void' is not a literal type"
//@ diag: 35:17 error "constexpr constructor's 1st parameter type 'NL' is not a literal type"
//@ diag: 25:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 36:16 error "constexpr function's return type 'NL' is not a literal type"
//@ diag: 25:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"
//@ diag: 38:53 error "constexpr function's 2nd parameter type 'NL' is not a literal type"
//@ diag: 25:8 note "'NL' is not literal because it is not an aggregate and has no constexpr constructors other than copy or move constructors"

struct NL { NL(); };
struct D { constexpr D() {} ~D() {} };
struct Lit { int a; };
struct Fwd;

constexpr NL ret() { return NL(); }
constexpr int param(int, NL) { return 1; }
constexpr int dtor_param(D) { return 1; }
constexpr void void_ret() {}
struct C {
  constexpr C(NL) {}
  constexpr NL member() const { return NL(); }
};
template <class T> constexpr int non_dependent(T, NL) { return 1; }

// Valid: literal types, a dependent type checked only on instantiation
// (clang does not diagnose the specialization either), and a declaration
// whose definition is not seen.
constexpr Lit lit(Lit l) { return l; }
template <class T> constexpr T dependent(T t) { return t; }
int use = (dependent(NL()), 0);
constexpr int fwd(Fwd *);
constexpr int unnamed_ok(int) { return 0; }

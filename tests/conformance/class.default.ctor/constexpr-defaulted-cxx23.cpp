// [class.default.ctor]/4, [dcl.constexpr] (C++23, P2448R2): a defaulted
// default constructor is constexpr whatever its subobjects; a
// non-constexpr member or base constructor fails only when a constant
// evaluation reaches it, with the defaulted constructor in the call stack.
//@ std: c++23
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:15 error "static assertion expression is not an integral constant expression"
//@ diag: 24:55 note "non-constexpr constructor 'NC' cannot be used in a constant expression"
//@ diag: 31:28 note "in call to 'M()'"
//@ diag: 32:15 note "in call to 'member()'"
//@ diag: 23:13 note "declared here"
//@ diag: 34:15 error "static assertion expression is not an integral constant expression"
//@ diag: 25:55 note "non-constexpr constructor 'NC' cannot be used in a constant expression"
//@ diag: 33:26 note "in call to 'B()'"
//@ diag: 34:15 note "in call to 'base()'"
//@ diag: 23:13 note "declared here"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:8 note "non-constexpr constructor 'NC' cannot be used in a constant expression"
//@ diag: 35:32 note "in call to 'Imp()'"
//@ diag: 36:15 note "in call to 'implicit()'"
//@ diag: 23:13 note "declared here"
struct NC { NC() {} constexpr NC(int) {} };
struct M { NC m; constexpr M(int) : m(0) {} constexpr M() = default; };
struct B : NC { constexpr B(int) : NC(0) {} constexpr B() = default; };
struct I { int m; constexpr I(int) : m(0) {} constexpr I() = default; };
union U { int a; float f; constexpr U(int) : a(0) {} constexpr U() = default; };
struct Imp { NC m; };
constexpr int ok() { I x; U u; return 0; }
static_assert(ok() == 0);
constexpr int member() { M x; return 0; }
static_assert(member() == 0);
constexpr int base() { B x; return 0; }
static_assert(base() == 0);
constexpr int implicit() { Imp x; return 0; }
static_assert(implicit() == 0);

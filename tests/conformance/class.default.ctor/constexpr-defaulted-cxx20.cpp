// [class.default.ctor]/4, [dcl.constexpr]/3 (C++20): a defaulted default
// constructor is constexpr only when every subobject's default
// initialization is (clang DefaultedDefaultConstructorIsConstexpr); an
// explicitly constexpr one that is not is an error before C++23. An
// uninitialized member or a union is fine (P1331). A volatile member makes
// the class non-literal, so a local of it in a constexpr function is an
// error before C++23 ([dcl.constexpr]/3).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 16:45 error "defaulted definition of default constructor cannot be marked constexpr before C++23"
//@ diag: 17:45 error "defaulted definition of default constructor cannot be marked constexpr before C++23"
//@ diag: 23:27 error "variable of non-literal type 'Vol' cannot be defined in a constexpr function before C++23"
//@ diag: 20:27 note "'Vol' is not literal because it has data member 'm' of volatile type 'volatile int'"
struct NC { NC() {} constexpr NC(int) {} };
struct M { NC m; constexpr M(int) : m(0) {} constexpr M() = default; };
struct B : NC { constexpr B(int) : NC(0) {} constexpr B() = default; };
struct I { int m; constexpr I(int) : m(0) {} constexpr I() = default; };
union U { int a; float f; constexpr U(int) : a(0) {} constexpr U() = default; };
struct Vol { volatile int m = 0; constexpr Vol() = default; };
constexpr int ok() { I x; U u; return 0; }
static_assert(ok() == 0);
constexpr int vol() { Vol v; return 0; }

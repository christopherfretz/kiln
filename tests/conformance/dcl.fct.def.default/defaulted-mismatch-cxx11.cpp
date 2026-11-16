// [dcl.fct.def.default]/2: before C++20 a defaulted special member whose
// type differs from the implicit declaration's is an error rather than
// deleted (P0641): a volatile parameter, a const move parameter, a const
// copy parameter a member requires non-const, and a const assignment (in
// C++11 a constexpr member function is implicitly const). Found running
// GCC's C++ testsuite.
//@ std: c++11
//@ args: -Wno-constexpr-not-const
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:22 error "the parameter for an explicitly-defaulted copy constructor may not be volatile"
//@ diag: 21:15 error "the parameter for an explicitly-defaulted copy assignment operator may not be volatile"
//@ diag: 22:12 error "the parameter for an explicitly-defaulted move constructor may not be const"
//@ diag: 23:15 error "the parameter for an explicitly-defaulted move assignment operator may not be const"
//@ diag: 24:12 error "the parameter for this explicitly-defaulted copy constructor is const, but a member or base requires it to be non-const"
//@ diag: 25:15 error "the parameter for this explicitly-defaulted copy assignment operator is const, but a member or base requires it to be non-const"
//@ diag: 26:15 error "an explicitly-defaulted copy assignment operator may not have 'const', 'constexpr' or 'volatile' qualifiers"
//@ diag: 27:25 error "an explicitly-defaulted copy assignment operator may not have 'const', 'constexpr' or 'volatile' qualifiers"
struct M { M(M&); M& operator=(M&); };
struct A { constexpr A(volatile A&) = default; };
struct B { B& operator=(volatile B&) = default; };
struct C { C(const C&&) = default; };
struct D { D& operator=(const D&&) = default; };
struct E { E(const E&) = default; M m; };
struct F { F& operator=(const F&) = default; M m; };
struct G { G& operator=(const G&) const = default; };
struct H { constexpr H& operator=(const H&) = default; };

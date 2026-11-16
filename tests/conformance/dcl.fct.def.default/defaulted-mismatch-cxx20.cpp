// [dcl.fct.def.default]/2 (C++20, P0641): a special member defaulted on
// its first declaration with a type that differs from the implicit
// declaration's is deleted, not an error; only its use is.
//@ std: c++20
//@ expect: compile-ok
//@ strict-diags
//@ diag: 16:12 warning "explicitly defaulted move constructor is implicitly deleted [-Wdefaulted-function-deleted]"
//@ diag: 16:12 note "function is implicitly deleted because its declared type does not match the type of an implicit move constructor"
//@ diag: 17:12 warning "explicitly defaulted copy constructor is implicitly deleted [-Wdefaulted-function-deleted]"
//@ diag: 17:12 note "function is implicitly deleted because its declared type does not match the type of an implicit copy constructor"
//@ diag: 18:15 warning "explicitly defaulted copy assignment operator is implicitly deleted [-Wdefaulted-function-deleted]"
//@ diag: 18:15 note "function is implicitly deleted because its declared type does not match the type of an implicit copy assignment operator"
//@ diag: 20:12 warning "explicitly defaulted copy constructor is implicitly deleted [-Wdefaulted-function-deleted]"
//@ diag: 20:12 note "function is implicitly deleted because its declared type does not match the type of an implicit copy constructor"
struct M { M(M&); M& operator=(M&); };
struct C { C(const C&&) = default; };
struct E { E(const E&) = default; M m; };
struct F { F& operator=(const F&) = default; M m; };
struct H { constexpr H& operator=(const H&) = default; };
struct A { A(volatile A&) = default; };
static_assert(!__is_constructible(E, const E&), "");
static_assert(__is_assignable(H&, const H&), "");

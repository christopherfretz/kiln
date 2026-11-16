// [expr.const]/22: a pointer in the result of a constant expression (in a
// member, an array element, a base, the active union member, one past an
// object, a string_view-like pair) may not point into a frame that has
// returned, nor to an allocation that leaked or was freed; a pointer to a
// member and a temporary the variable extends are fine. Found running
// GCC's C++ testsuite (WP-1468).
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 42:42 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 43:13 error "constexpr variable 'g1' must be initialized by a constant expression"
//@ diag: 43:13 note "pointer to 'x' is not a constant expression"
//@ diag: 42:26 note "declared here"
//@ diag: 44:44 warning "reference to stack memory associated with local variable 'x' returned"
//@ diag: 45:55 warning "address of stack memory associated with local variable 'x' returned"
//@ diag: 46:15 error "constexpr variable 'g3' must be initialized by a constant expression"
//@ diag: 46:15 note "pointer to 'x' is not a constant expression"
//@ diag: 45:29 note "declared here"
//@ diag: 48:15 error "constexpr variable 'g4' must be initialized by a constant expression"
//@ diag: 48:15 note "pointer to 'x' is not a constant expression"
//@ diag: 47:29 note "declared here"
//@ diag: 50:13 error "constexpr variable 'g5' must be initialized by a constant expression"
//@ diag: 50:13 note "pointer to 'x' is not a constant expression"
//@ diag: 49:27 note "declared here"
//@ diag: 51:53 warning "address of stack memory associated with local variable 'buf' returned"
//@ diag: 52:14 error "constexpr variable 'g6' must be initialized by a constant expression"
//@ diag: 52:14 note "pointer to subobject of 'buf' is not a constant expression"
//@ diag: 51:29 note "declared here"
//@ diag: 54:13 error "constexpr variable 'g7' must be initialized by a constant expression"
//@ diag: 54:13 note "pointer to heap-allocated object is not a constant expression"
//@ diag: 53:30 note "heap allocation performed here"
//@ diag: 56:13 error "constexpr variable 'g8' must be initialized by a constant expression"
//@ diag: 56:13 note "pointer to heap-allocated object is not a constant expression"
struct P { const int *p; };
struct R { const int &r; };
struct Arr { const int *a[2]; };
struct Base { const int *b; };
struct Der : Base { int d; };
union U { const int *u; long l; };
struct SV { const char *s; unsigned long n; };
struct M { int x; };
constexpr P dead() { int x = 1; return {&x}; }
constexpr P g1 = dead();
constexpr R deadref() { int x = 1; return {x}; }
constexpr Arr dead2() { int x = 1; return {{nullptr, &x}}; }
constexpr Arr g3 = dead2();
constexpr Der dead3() { int x = 1; Der d{}; d.b = &x; return d; }
constexpr Der g4 = dead3();
constexpr U dead4() { int x = 1; U u{}; u.u = &x + 1; return u; }
constexpr U g5 = dead4();
constexpr SV dead5() { char buf[4] = "abc"; return {buf, 3}; }
constexpr SV g6 = dead5();
constexpr P leak() { return {new int(1)}; }
constexpr P g7 = leak();
constexpr P freed() { int *q = new int(1); delete q; return {q}; }
constexpr P g8 = freed();
constexpr int M::*mp = &M::x;
static_assert(mp == &M::x);
constexpr const int &ext = 5;
constexpr P g9 = {&ext};
static_assert(*g9.p == 5);

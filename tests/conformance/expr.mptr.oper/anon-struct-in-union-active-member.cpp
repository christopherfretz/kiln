// [expr.mptr.oper]/6, [class.union.anon], [expr.const]/10.9: a pointer to a
// member of an anonymous struct inside an anonymous union designates that
// member in constant evaluation; reading or writing through it needs the
// anonymous struct to be the union's active member. Found running GCC's C++
// testsuite (WP-1497).
//@ expect: compile-fail
//@ std: c++20
//@ diag: 13:15 error "constexpr variable 'bad' must be initialized by a constant expression"
//@ diag: 15:15 error "constexpr variable 'bad2' must be initialized by a constant expression"
struct U { int x; union { struct { int a; int b; }; int c; }; };
constexpr int rd(int U::*p) { U u{}; u.x = 1; u.b = 5; return u.*p; }
static_assert(rd(&U::b) == 5 && rd(&U::a) == 0 && rd(&U::x) == 1);
constexpr int bad = rd(&U::c);
constexpr int wr(int U::*p) { U u{}; u.c = 3; u.*p = 7; return u.c; }
constexpr int bad2 = wr(&U::b);
constexpr int ok(int U::*p) { U u{}; u.a = 3; u.*p = 7; return u.a + u.b; }
static_assert(ok(&U::b) == 10);

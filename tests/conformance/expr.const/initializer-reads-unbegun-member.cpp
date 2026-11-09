// [expr.const]/10.9, [basic.life]/7: in a variable's own initializer, a
// member not yet initialized has not begun its lifetime: reading it through
// the variable's name is not a constant expression (invalid twin of
// initializer-reads-own-member.cpp). WP-1100.
//@ std: c++26
//@ expect: compile-fail
//@ strict-diags
//@ diag: 17:13 error "constexpr variable 'bad' must be initialized by a constant expression"
//@ diag: 17:17 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 19:13 error "constexpr variable 'nested' must be initialized by a constant expression"
//@ diag: 19:21 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 21:13 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 21:18 note "read of object outside its lifetime is not allowed in a constant expression"
//@ diag: 23:13 error "constexpr variable 'v' must be initialized by a constant expression"
//@ diag: 22:31 note "read of object outside its lifetime is not allowed in a constant expression"
struct P { int x; int y; };
constexpr P bad{bad.y, 1};
struct Q { P a; int b; };
constexpr Q nested{{nested.b, 1}, 2};
struct C { int x; int y; constexpr C(int a, int b) : x(a), y(b) {} };
constexpr C c(1, c.x);
struct V { int a = 1; int c = this->d; int d = 4; };
constexpr V v{};

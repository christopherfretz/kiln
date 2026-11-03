// [class.copy.assign], [dcl.fct]/6 (P0847): `operator=` with an explicit
// object parameter is an assignment operator; a non-constexpr one makes
// the call not a constant expression. Matches clang 19 and 22.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:15 error "static assertion expression is not an integral constant expression"
//@ diag: 14 note "non-constexpr function 'operator=' cannot be used in a constant expression"
//@ diag: 15:15 note "in call to 'g()'"
//@ diag: 13:22 note "declared here"
struct A { int v; constexpr A& operator=(this A& self, int x) { self.v = x; return self; } };
constexpr int f() { A a{1}; a = 5; return a.v; }
static_assert(f() == 5);
struct N { int v; N& operator=(this N& self, int x) { self.v = x; return self; } };
constexpr int g() { N a{1}; a = 5; return a.v; }
static_assert(g() == 5);

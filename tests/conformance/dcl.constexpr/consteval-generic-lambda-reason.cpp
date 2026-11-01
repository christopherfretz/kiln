// [expr.const]/20-21 (P2564, WP-158 item d): the reason a generic
// lambda's escalated call operator specialization is immediate is named
// (unqualified) at its failed invocation; clang omits it for a
// non-template call operator, even one instantiated with its function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 32:24 error "call to immediate function '(anonymous class)::operator()' is not a constant expression"
//@ diag: 32:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 32:12 note "declared here"
//@ diag: 34:24 error "call to immediate function '(anonymous class)::operator()<int>' is not a constant expression"
//@ diag: 33:30 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 34:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 34:12 note "declared here"
//@ diag: 36:24 error "call to immediate function '(anonymous class)::operator()<int>' is not a constant expression"
//@ diag: 35:36 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 36:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 36:12 note "declared here"
//@ diag: 37:63 error "call to immediate function 'r4(int)::(anonymous class)::operator()<int>' is not a constant expression"
//@ diag: 37:46 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 37:65 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 37:12 note "declared here"
//@ diag: 42:24 error "call to immediate function '(anonymous class)::operator()<int>' is not a constant expression"
//@ diag: 40:60 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 42:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 42:12 note "declared here"
//@ diag: 38:79 error "call to immediate function 'r5(int)::(anonymous class)::operator()' is not a constant expression"
//@ diag: 39:10 note "in instantiation of function template specialization 'r5<int>' requested here"
//@ diag: 38:81 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 38:29 note "declared here"
consteval int ce(int x) { return x; }
auto l = [](int x) { return ce(x); };
int r1(int y) { return l(y); }
auto g = [](auto x) { return ce(x); };
int r2(int y) { return g(y); }
auto t = []<class T>(T x) { return ce(x); };
int r3(int y) { return t(y); }
int r4(int y) { auto q = [](auto x) { return ce(x); }; return q(y); }
template <class U> int r5(U y) { auto q = [](int x) { return ce(x); }; return q(y); }
int i5 = r5(1);
auto h = [](auto x) { if constexpr (sizeof(x) == 4) return ce(x); else return x; };
long r6(long y) { return h(y); }
int r7(int y) { return h(y); }

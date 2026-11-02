// [expr.const]/20-21 (P2564, WP-158 item d): the reason a generic
// lambda's escalated call operator specialization is immediate is named
// (unqualified) at its failed invocation; clang omits it for a
// non-template call operator, even one instantiated with its function.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 33:24 error "call to immediate function '(lambda)::operator()' is not a constant expression"
//@ diag: 33:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 33:12 note "declared here"
//@ diag: 35:24 error "call to immediate function '(lambda)::operator()<int>' is not a constant expression"
//@ diag: 34:30 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 35:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 35:12 note "declared here"
//@ diag: 37:24 error "call to immediate function '(lambda)::operator()<int>' is not a constant expression"
//@ diag: 36:36 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 37:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 37:12 note "declared here"
//@ diag: 38:63 error "call to immediate function 'r4(int)::(lambda)::operator()<int>' is not a constant expression"
//@ diag: 38:46 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 38:65 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 38:12 note "declared here"
//@ diag: 43:24 error "call to immediate function '(lambda)::operator()<int>' is not a constant expression"
//@ diag: 41:60 note "'operator()<int>' is an immediate function because its body contains a call to a consteval function 'ce' and that call is not a constant expression"
//@ diag: 43:26 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 43:12 note "declared here"
//@ diag: 39:79 error "call to immediate function 'r5(int)::(lambda)::operator()' is not a constant expression"
//@ diag: 40:10 note "in instantiation of function template specialization 'r5<int>' requested here"
//@ diag: 39:81 note "function parameter 'y' with unknown value cannot be used in a constant expression"
//@ diag: 39:29 note "declared here"
//@ requires: clang>=22
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

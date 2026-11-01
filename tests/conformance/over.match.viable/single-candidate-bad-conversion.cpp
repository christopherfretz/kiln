// [over.match.viable]/4, [over.best.ics]/1: an unqualified call with one candidate whose argument
// has no implicit conversion sequence is 'no matching function', noting the bad conversion with
// the parameter type (an explicit ctor, a missing ctor, a reference binding, a lost qualifier)
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:12 error "no matching function for call to 'h'"
//@ diag: 23:6 note "candidate function not viable: no known conversion from 'int' to 'C' for 1st argument"
//@ diag: 25:18 error "no matching function for call to 'm'"
//@ diag: 23:17 note "candidate function not viable: no known conversion from 'int' to 'D' for 1st argument"
//@ diag: 25:24 error "no matching function for call to 'r'"
//@ diag: 23:28 note "candidate function not viable: expects an lvalue for 1st argument"
//@ diag: 25:30 error "no matching function for call to 's'"
//@ diag: 23:42 note "candidate function not viable: no known conversion from 'std::nullptr_t' to 'const int' for 1st argument"
//@ diag: 25:42 error "no matching function for call to 'u'"
//@ diag: 23:62 note "candidate function not viable: no known conversion from 'int' to 'const C' for 1st argument"
//@ diag: 25:48 error "no matching function for call to 'v'"
//@ diag: 23:80 note "candidate function not viable: no known conversion from 'int' to 'C' for 1st argument"
//@ diag: 25:54 error "no matching function for call to 'w'"
//@ diag: 23:93 note "candidate function not viable: 1st argument ('const E') would lose const qualifier"
struct C { explicit C(const int&); };
struct D { D(char*); };
struct E { };
void h(C); void m(D); void r(int&); void s(const int&); void u(const C&); void v(C&&); void w(E&);
const E ce{};
void k() { h(1); m(1); r(1); s(nullptr); u(1); v(1); w(ce); }

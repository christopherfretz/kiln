// [expr.const]/17-20: taking the address of an escalated function outside
// an immediate invocation is an error whose notes say why it is immediate.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:19 error "cannot take address of immediate function 'a<int>' outside of an immediate invocation"
//@ diag: 10:34 note "declared here"
//@ diag: 10:50 note "'a<int>' is an immediate function because its body contains a call to a consteval function 'sq' and that call is not a constant expression"
consteval int sq(int x) { return x * x; }
int g;
template <class T> constexpr int a(T y) { return sq(g); }
int (*fa)(int) = &a<int>;

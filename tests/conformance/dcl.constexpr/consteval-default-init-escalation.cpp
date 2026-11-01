// [expr.const]/18-20 (WP-137): in an immediate-escalating function a
// default argument or default member initializer whose immediate
// invocation is not a constant expression makes the function immediate:
// a defaulted or implicit constructor ("an immediate constructor because
// the default initializer of"), a lambda's call operator (silently), a
// constexpr function template specialization. The call of the escalated
// function is then reported.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 39:16 error "call to immediate function 'D::D' is not a constant expression"
//@ diag: 37:26 note "'D' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 37:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 39:16 note "in call to 'D()'"
//@ diag: 35:5 note "declared here"
//@ diag: 40:16 error "call to immediate function 'S::S' is not a constant expression"
//@ diag: 38:8 note "'S' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 38:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 40:16 note "in call to 'S()'"
//@ diag: 35:5 note "declared here"
//@ diag: 41:21 error "call to immediate function 'S::S' is not a constant expression"
//@ diag: 38:8 note "'S' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 38:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 41:21 note "in call to 'S()'"
//@ diag: 35:5 note "declared here"
//@ diag: 43:21 error "call to immediate function 'tc<int>' is not a constant expression"
//@ diag: 36:15 note "'tc<int>' is an immediate function because its body contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 36:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 43:21 note "in call to 'tc<int>()'"
//@ diag: 35:5 note "declared here"
//@ diag: 45:21 error "call to immediate function '(anonymous class)::operator()' is not a constant expression"
//@ diag: 36:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 45:21 note "in call to 'l.operator()()'"
//@ diag: 35:5 note "declared here"
consteval int h(int x) { return x; }
int k;
int f(int p = h(k));
struct D { int m = h(k); D() = default; };
struct S { int m = h(k); int n = 1; };
int use1() { D d; return d.m; }
int use2() { S s; return s.n; }
int use3() { return S().n; }
template <class T> constexpr int tc() { return f(); }
int use4() { return tc<int>(); }
auto l = [] { return f(); };
int use5() { return l(); }
consteval int use6() { D d; return f(1) + d.m; }

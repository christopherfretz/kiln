// [expr.const]/18-20 (WP-137): in an immediate-escalating function a
// default argument or default member initializer whose immediate
// invocation is not a constant expression makes the function immediate:
// a defaulted or implicit constructor ("an immediate constructor because
// the default initializer of"), a lambda's call operator (silently), a
// constexpr function template specialization. The call of the escalated
// function is then reported.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 40:16 error "call to immediate function 'D::D' is not a constant expression"
//@ diag: 38:26 note "'D' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 38:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 40:16 note "in call to 'D()'"
//@ diag: 36:5 note "declared here"
//@ diag: 41:16 error "call to immediate function 'S::S' is not a constant expression"
//@ diag: 39:8 note "'S' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 39:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 41:16 note "in call to 'S()'"
//@ diag: 36:5 note "declared here"
//@ diag: 42:21 error "call to immediate function 'S::S' is not a constant expression"
//@ diag: 39:8 note "'S' is an immediate constructor because the default initializer of 'm' contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 39:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 42:21 note "in call to 'S()'"
//@ diag: 36:5 note "declared here"
//@ diag: 44:21 error "call to immediate function 'tc<int>' is not a constant expression"
//@ diag: 37:15 note "'tc<int>' is an immediate function because its body contains a call to a consteval function 'h' and that call is not a constant expression"
//@ diag: 37:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 44:21 note "in call to 'tc<int>()'"
//@ diag: 36:5 note "declared here"
//@ diag: 46:21 error "call to immediate function '(lambda)::operator()' is not a constant expression"
//@ diag: 37:17 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 46:21 note "in call to 'l.operator()()'"
//@ diag: 36:5 note "declared here"
//@ requires: clang>=22
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

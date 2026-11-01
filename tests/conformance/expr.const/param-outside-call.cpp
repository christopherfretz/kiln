//@ strict-diags
//@ expect: compile-fail
//@ diag: 34:19 error "non-type template argument is not a constant expression"
//@ diag: 34:19 note "pointer to 'q' is not a constant expression"
//@ diag: 34:12 note "declared here"
//@ diag: 35:19 error "non-type template argument is not a constant expression"
//@ diag: 35:19 note "reference to 'q' is not a constant expression"
//@ diag: 35:12 note "declared here"
//@ diag: 36:25 error "non-type template argument is not a constant expression"
//@ diag: 36:25 note "pointer to 'q' is not a constant expression"
//@ diag: 36:16 note "declared here"
//@ diag: 37:19 error "non-type template argument is not a constant expression"
//@ diag: 37:19 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 37:12 note "declared here"
//@ diag: 38:20 error "non-type template argument is not a constant expression"
//@ diag: 38:21 note "function parameter 'r' with unknown value cannot be used in a constant expression"
//@ diag: 38:13 note "declared here"
//@ diag: 39:37 error "static assertion expression is not an integral constant expression"
//@ diag: 39:37 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 39:18 note "declared here"
//@ diag: 40:38 error "static assertion expression is not an integral constant expression"
//@ diag: 40:38 note "function parameter 'r' with unknown value cannot be used in a constant expression"
//@ diag: 40:19 note "declared here"
//@ diag: 41:23 error "non-type template argument is not a constant expression"
//@ diag: 41:23 note "function parameter 'q' with unknown value cannot be used in a constant expression"
//@ diag: 41:12 note "declared here"
// [expr.const]: a function parameter named outside its call (in a template
// argument or a static_assert) has no frame. Naming it is fine, so `&q`
// fails in the result check; only a read is "unknown value" (clang
// VisitVarDecl / note_constexpr_function_param_value_unknown).
template <int *P> struct X {};
template <const int &R> struct Y {};
template <int N> struct Z {};
void f(int q) { X<&q> x; }
void g(int q) { Y<q> y; }
void h() { int q = 0; X<&q> x; }
void i(int q) { Z<q> z; }
void j(int &r) { X<&r> x; }
void k(const int q) { static_assert(q == 0); }
void l(const int &r) { static_assert(r == 0); }
void m(int q = 3) { Z<q> z; }
void n(int q) { static_assert(sizeof(q) == 4); }

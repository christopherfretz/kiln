// [expr.const]/16 (WP-137): the immediate invocations of a default member
// initializer are evaluated where a constructor or an aggregate
// initialization uses it ("in the default initializer of": the
// constructor, or the closing brace); one a member initializer replaces is
// not used. A non-dependent use in a function template is checked again at
// each instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 38:20 error "call to consteval function 'h' is not a constant expression"
//@ diag: 38:40 note "in the default initializer of 'm'"
//@ diag: 38:12 note "declared here"
//@ diag: 38:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 36:5 note "declared here"
//@ diag: 39:20 error "call to consteval function 'h' is not a constant expression"
//@ diag: 39:36 note "in the default initializer of 'm'"
//@ diag: 39:12 note "declared here"
//@ diag: 39:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 36:5 note "declared here"
//@ diag: 41:20 error "call to consteval function 'h' is not a constant expression"
//@ diag: 42:24 note "in the default initializer of 'a'"
//@ diag: 41:12 note "declared here"
//@ diag: 41:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 36:5 note "declared here"
//@ diag: 41:20 error "call to consteval function 'h' is not a constant expression"
//@ diag: 44:41 note "in the default initializer of 'a'"
//@ diag: 41:12 note "declared here"
//@ diag: 41:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 36:5 note "declared here"
//@ diag: 41:20 error "call to consteval function 'h' is not a constant expression"
//@ diag: 45:21 note "in instantiation of function template specialization 'ta<int>' requested here"
//@ diag: 44:41 note "in the default initializer of 'a'"
//@ diag: 41:12 note "declared here"
//@ diag: 41:22 note "read of non-const variable 'k' is not allowed in a constant expression"
//@ diag: 36:5 note "declared here"
consteval int h(int x) { return x; }
int k;
constexpr int c = 4;
struct U { int m = h(k); int n = h(c); U() {} };
struct G { int m = h(k); constexpr G() {} };
struct T { int m = h(k); T() : m(1) {} };
struct A { int a = h(k); int b; };
int use1() { A x{.b = 1}; return x.a; }
int use2() { A x{1, 2}; return x.a; }
template <class V> int ta() { A x{.b = 1}; return x.a; }
int use3() { return ta<int>(); }

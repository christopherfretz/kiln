// [expr.const.imm]/2-3: the note explaining why an immediate-escalating
// function became immediate names the callee as clang 19 and 22 both do
// (`DiagnoseImmediateEscalatingReason`): unqualified (`sq`, `m`, not
// `N::sq`), and a lambda's call operator invoked in the body as "evaluates
// the address of an immediate function 'operator()'", not "contains a
// call to". Where clang 19 and 22 disagree on escalation itself (taking
// the address of a consteval function) is left out. WP-1361.
//@ expect: compile-fail
//@ std: c++26
//@ strict-diags
//@ diag: 33:23 error "call to immediate function 'k<int>' is not a constant expression"
//@ diag: 32:50 note "'k<int>' is an immediate function because its body contains a call to a consteval function 'sq' and that call is not a constant expression"
//@ diag: 33:25 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 33:11 note "declared here"
//@ diag: 35:24 error "call to immediate function 'k2<int>' is not a constant expression"
//@ diag: 34:51 note "'k2<int>' is an immediate function because its body contains a call to a consteval function 'm' and that call is not a constant expression"
//@ diag: 35:27 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 35:12 note "declared here"
//@ diag: 37:24 error "call to immediate function 'k4<int>' is not a constant expression"
//@ diag: 36:95 note "'k4<int>' is an immediate function because its body evaluates the address of a consteval function 'operator()'"
//@ diag: 37:27 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 37:12 note "declared here"
//@ diag: 39:24 error "call to immediate function 'k5<int>' is not a constant expression"
//@ diag: 38:51 note "'k5<int>' is an immediate function because its body evaluates the address of an immediate function 'operator()'"
//@ diag: 39:27 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 39:12 note "declared here"
//@ diag: 41:24 error "call to immediate function 'k8<int>' is not a constant expression"
//@ diag: 40:51 note "'k8<int>' is an immediate function because its body contains a call to a consteval function 'sq' and that call is not a constant expression"
//@ diag: 41:27 note "function parameter 'n' with unknown value cannot be used in a constant expression"
//@ diag: 41:12 note "declared here"
namespace N { consteval int sq(int x) { return x; } struct S { static consteval int m(int x) { return x; } }; }
template <class T> constexpr int k(T t) { return N::sq(t); }
int u(int n) { return k(n); }
template <class T> constexpr int k2(T t) { return N::S::m(t); }
int u2(int n) { return k2(n); }
template <class T> constexpr int k4(T t) { auto l = [](int v) consteval { return v; }; return l(t); }
int u4(int n) { return k4(n); }
template <class T> constexpr int k5(T t) { return [&] { return N::sq(t); }(); }
int u5(int n) { return k5(n); }
template <class T> constexpr int k8(T t) { return N::sq(N::sq(t)); }
int u8(int n) { return k8(n); }

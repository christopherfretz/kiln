// [expr.const]/22 (WP-137): a constant expression's result does not point
// to, refer to, or name as a member pointer an immediate function, so an
// immediate invocation cannot hand one out (clang's
// note_consteval_address_accessible). C++23: with C++26 reflection kiln
// follows P4101R1, which struck that rule, and a pointer to an immediate
// function is a consteval-only value instead
// (expr.const.const/p3-immediate-function-pointer-{run,neg}.cpp).
//@ expect: compile-fail
//@ std: c++23
//@ strict-diags
//@ diag: 26:7 error "call to consteval function 'get' is not a constant expression"
//@ diag: 26:7 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 23:15 note "declared here"
//@ diag: 27:18 error "call to consteval function 'get' is not a constant expression"
//@ diag: 27:18 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 23:15 note "declared here"
//@ diag: 28:33 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 28:33 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 23:15 note "declared here"
//@ diag: 35:9 error "call to consteval function 'gm' is not a constant expression"
//@ diag: 35:9 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 32:26 note "declared here"
consteval int f(int x) { return x; }
using P = int (*)(int);
consteval P get() { return f; }
P a = get();
void g() { P b = get(); }
consteval int h() { constexpr P x = get(); return x(1); }
consteval int h2() { P x = get(); return x(1); }
static_assert(h2() == 1);
constexpr int plain(int x) { return x; }
struct S { consteval int m() const { return 1; } };
using MP = int (S::*)() const;
consteval MP gm() { return &S::m; }
MP mp = gm();
consteval P ok() { return plain; }
P fine = ok();

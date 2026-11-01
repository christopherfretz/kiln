// [expr.const]/22 (WP-137): a constant expression's result does not point
// to, refer to, or name as a member pointer an immediate function, so an
// immediate invocation cannot hand one out (clang's
// note_consteval_address_accessible).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:7 error "call to consteval function 'get' is not a constant expression"
//@ diag: 22:7 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 19:15 note "declared here"
//@ diag: 23:18 error "call to consteval function 'get' is not a constant expression"
//@ diag: 23:18 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 19:15 note "declared here"
//@ diag: 24:33 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 24:33 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 19:15 note "declared here"
//@ diag: 31:9 error "call to consteval function 'gm' is not a constant expression"
//@ diag: 31:9 note "pointer to a consteval declaration is not a constant expression"
//@ diag: 28:26 note "declared here"
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

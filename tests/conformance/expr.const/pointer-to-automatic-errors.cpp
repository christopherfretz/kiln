// [expr.const]/14: a constant expression's pointer or reference value
// must not designate an object with automatic storage duration, even
// when the variable is static or the referent is const-initialized.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 23:49 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 23:49 note "pointer to 'local' is not a constant expression"
//@ diag: 23:16 note "declared here"
//@ diag: 24:42 error "constexpr variable 'r' must be initialized by a constant expression"
//@ diag: 24:42 note "reference to 'local' is not a constant expression"
//@ diag: 24:16 note "declared here"
//@ diag: 25:54 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 25:54 note "pointer to 'local' is not a constant expression"
//@ diag: 25:22 note "declared here"
//@ diag: 26:32 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 26:32 note "pointer to 'x' is not a constant expression"
//@ diag: 26:12 note "declared here"
//@ diag: 27:46 error "constexpr variable 't' must be initialized by a constant expression"
//@ diag: 27:46 note "pointer to 'local' is not a constant expression"
//@ diag: 27:16 note "declared here"
int k;
struct T { int *p; };
void f() { int local = 0; static constexpr int *p = &local; (void)p; }
void g() { int local = 0; constexpr int &r = local; (void)r; }
void h() { const int local = 5; constexpr const int *p = &local; (void)p; }
void i(int x) { constexpr int *p = &x; (void)p; }
void j() { int local = 0; static constexpr T t{&local}; (void)t; }
void l() { static int s; constexpr int *p = &s; constexpr int *q = &k; (void)p; (void)q; }

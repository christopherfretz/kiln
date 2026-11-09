// [dcl.constexpr]/6, [expr.const]/10: under GNU modes `bcmp` is a library
// builtin clang constant-evaluates, like `memcmp`, so a call is noted as
// a non-constexpr function whatever its arguments, with no "declared
// here". `strdup` and `index` (GNU library builtins clang does not
// evaluate) fail only with known arguments, with a "declared here" note.
// WP-1055; the c++20 twin has `bcmp` as an ordinary function. spec/sema.md
// "Never constant".
//@ args: -std=gnu++20
//@ strict-diags
//@ expect: compile-fail
//@ diag: 27:15 error "constexpr function never produces a constant expression"
//@ diag: 27:49 note "non-constexpr function 'bcmp' cannot be used in a constant expression"
//@ diag: 28:15 error "constexpr function never produces a constant expression"
//@ diag: 28:29 note "non-constexpr function 'bcmp' cannot be used in a constant expression"
//@ diag: 29:15 error "constexpr function never produces a constant expression"
//@ diag: 29:49 note "non-constexpr function 'memcmp' cannot be used in a constant expression"
//@ diag: 30:16 error "constexpr function never produces a constant expression"
//@ diag: 30:30 note "non-constexpr function 'strdup' cannot be used in a constant expression"
//@ diag: 25:18 note "declared here"
//@ diag: 32:16 error "constexpr function never produces a constant expression"
//@ diag: 32:30 note "non-constexpr function 'index' cannot be used in a constant expression"
//@ diag: 26:18 note "declared here"
extern "C" int bcmp(const void *, const void *, decltype(sizeof 0));
extern "C" int memcmp(const void *, const void *, decltype(sizeof 0));
extern "C" char *strdup(const char *);
extern "C" char *index(const char *, int);
constexpr int b1(const void *p, int x) { return bcmp(p, p, 1) + x; }
constexpr int b2() { return bcmp("a", "a", 1); }
constexpr int m1(const void *p, int x) { return memcmp(p, p, 1) + x; }
constexpr bool d1() { return strdup("a") != nullptr; }
constexpr bool d2(const char *s) { return strdup(s) != nullptr; }
constexpr bool i1() { return index("ab", 'b') != nullptr; }

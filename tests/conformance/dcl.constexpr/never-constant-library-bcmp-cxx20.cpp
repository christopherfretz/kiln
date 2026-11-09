// [dcl.constexpr]/6, [expr.const]/10: outside GNU modes `bcmp` is not a
// library builtin (clang's GnuMode `LIBBUILTIN`), so a call is an ordinary
// non-constexpr function: only one with known arguments fails, with a
// "declared here" note. `memcmp` is still constant-evaluated by clang and
// noted at any call; `strdup` and `index` are ordinary functions here and
// fail only with known arguments, as in GNU modes. WP-1055; the gnu++20
// twin. spec/sema.md "Never constant".
//@ std: c++20
//@ strict-diags
//@ expect: compile-fail
//@ diag: 27:15 error "constexpr function never produces a constant expression"
//@ diag: 27:29 note "non-constexpr function 'bcmp' cannot be used in a constant expression"
//@ diag: 22:16 note "declared here"
//@ diag: 28:15 error "constexpr function never produces a constant expression"
//@ diag: 28:49 note "non-constexpr function 'memcmp' cannot be used in a constant expression"
//@ diag: 29:16 error "constexpr function never produces a constant expression"
//@ diag: 29:30 note "non-constexpr function 'strdup' cannot be used in a constant expression"
//@ diag: 24:18 note "declared here"
//@ diag: 31:16 error "constexpr function never produces a constant expression"
//@ diag: 31:30 note "non-constexpr function 'index' cannot be used in a constant expression"
//@ diag: 25:18 note "declared here"
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

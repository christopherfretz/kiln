// [expr.const]/5, [cwchar.syn], [cstring.syn]: `wmemcpy`/`wmemmove` count
// wide characters and report the `memcpy` faults under their own names;
// `bcmp` compares only narrow characters, as `memcmp`; `memset` and
// `mempcpy` are not constant-evaluable at all (as in clang 19).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 35:15 error "static assertion expression is not an integral constant expression"
//@ diag: 28:45 note "'wmemcpy' between overlapping memory regions"
//@ diag: 35:15 note "in call to 'e1()'"
//@ diag: 36:15 error "static assertion expression is not an integral constant expression"
//@ diag: 29:56 note "'wmemcpy' not supported: destination is not a contiguous array of at least 3 elements of type 'wchar_t'"
//@ diag: 36:15 note "in call to 'e2()'"
//@ diag: 37:15 error "static assertion expression is not an integral constant expression"
//@ diag: 30:63 note "constant evaluation of '__builtin_bcmp' between arrays of types 'int' and 'int' is not supported; only arrays of narrow character types can be compared"
//@ diag: 37:15 note "in call to 'e3()'"
//@ diag: 38:15 error "static assertion expression is not an integral constant expression"
//@ diag: 31:45 note "destination of 'wmemmove' is nullptr"
//@ diag: 38:15 note "in call to 'e4()'"
//@ diag: 39:15 error "static assertion expression is not an integral constant expression"
//@ diag: 32:45 note "'wmemmove' not supported: source is not a contiguous array of at least 9 elements of type 'wchar_t'"
//@ diag: 39:15 note "in call to 'e5()'"
//@ diag: 40:15 error "static assertion expression is not an integral constant expression"
//@ diag: 33:41 note "subexpression not valid in a constant expression"
//@ diag: 40:15 note "in call to 'e6()'"
//@ diag: 41:15 error "static assertion expression is not an integral constant expression"
//@ diag: 34:52 note "subexpression not valid in a constant expression"
//@ diag: 41:15 note "in call to 'e7()'"
constexpr int e1() { wchar_t a[4] = L"abc"; __builtin_wmemcpy(a + 1, a, 2); return a[0]; }
constexpr int e2() { wchar_t a[4] = L"abc", b[2] = {}; __builtin_wmemcpy(b, a, 3); return b[0]; }
constexpr int e3() { int a[2] = {1, 2}, b[2] = {1, 3}; return __builtin_bcmp(a, b, 8); }
constexpr int e4() { wchar_t a[4] = L"abc"; __builtin_wmemmove(nullptr, a, 1); return 0; }
constexpr int e5() { wchar_t a[4] = L"abc"; __builtin_wmemmove(a, a + 1, 9); return 0; }
constexpr int e6() { char a[4] = "abc"; __builtin_memset(a, 0, 1); return a[0]; }
constexpr int e7() { char a[4] = "abc", b[4] = {}; __builtin_mempcpy(b, a, 2); return b[1]; }
static_assert(e1() == 0);
static_assert(e2() == 0);
static_assert(e3() == 0);
static_assert(e4() == 0);
static_assert(e5() == 0);
static_assert(e6() == 0);
static_assert(e7() == 'b');

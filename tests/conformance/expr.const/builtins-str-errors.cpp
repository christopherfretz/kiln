// [expr.const]/5, [cstring.syn], [cwchar.syn]: the string builtins reject
// what is not a constant as clang 19 does: a null pointer, a read past the
// end of an unterminated array; strrchr, strstr and strpbrk are not
// constant-evaluable at all (no note at the top level, "subexpression not
// valid" inside a call).
//@ expect: compile-fail
//@ strict-diags
//@ diag: 26:15 error "constexpr variable 'e1' must be initialized by a constant expression"
//@ diag: 26:20 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 27:15 error "constexpr variable 'e2' must be initialized by a constant expression"
//@ diag: 27:20 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 28:15 error "constexpr variable 'e3' must be initialized by a constant expression"
//@ diag: 28:20 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 29:23 error "constexpr variable 'e4' must be initialized by a constant expression"
//@ diag: 29:28 note "read of dereferenced one-past-the-end pointer is not allowed in a constant expression"
//@ diag: 30:26 error "constexpr variable 'e5' must be initialized by a constant expression"
//@ diag: 30:31 note "read of dereferenced null pointer is not allowed in a constant expression"
//@ diag: 31:28 error "constexpr variable 'e6' must be initialized by a constant expression"
//@ diag: 32:28 error "constexpr variable 'e7' must be initialized by a constant expression"
//@ diag: 33:28 error "constexpr variable 'e8' must be initialized by a constant expression"
//@ diag: 35:23 error "constexpr variable 'e9' must be initialized by a constant expression"
//@ diag: 34:36 note "subexpression not valid in a constant expression"
//@ diag: 35:28 note "in call to 'f()'"
typedef decltype(sizeof 0) size_t;
constexpr char a3[3] = {'a', 'b', 'c'};
constexpr int e1 = __builtin_strcmp("abc", nullptr);
constexpr int e2 = __builtin_strcmp(a3, "abc");
constexpr int e3 = __builtin_strncmp(a3, "abcd", 4);
constexpr const char *e4 = __builtin_strchr(a3, 'z');
constexpr const wchar_t *e5 = __builtin_wcschr(nullptr, L'a');
constexpr const char *e6 = __builtin_strrchr("abc", 'a');
constexpr const char *e7 = __builtin_strstr("abc", "b");
constexpr const char *e8 = __builtin_strpbrk("abc", "b");
constexpr const char *f() { return __builtin_strstr("ab", "b"); }
constexpr const char *e9 = f();

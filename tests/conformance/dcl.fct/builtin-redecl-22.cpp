// [dcl.fct]/1, [dcl.link]/6: at -fclang-compat=22 a file-scope `extern
// "C"` declaration of a builtin's name differing from the builtin only in
// the return type is "conflicting types" too (clang 22; clang 19:
// builtin-redecl-19.cpp), and the builtin-type note spells `size_t` as
// clang 22's `__size_t` (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang>=22
//@ diag: 15:16 error "conflicting types for '__builtin_cos'"
//@ diag: 15:16 note "'__builtin_cos' is a builtin with type 'double (double) noexcept'"
//@ diag: 16:18 error "conflicting types for '__builtin_memset'"
//@ diag: 16:18 note "'__builtin_memset' is a builtin with type 'void *(void *, int, __size_t) noexcept' (aka 'void *(void *, int, unsigned long) noexcept')"
//@ diag: 17:16 error "conflicting types for '__builtin_expect'"
//@ diag: 17:16 note "'__builtin_expect' is a builtin with type 'long (long, long) noexcept'"
extern "C" int __builtin_cos(double);
extern "C" void *__builtin_memset(void *, int, int);
extern "C" int __builtin_expect(long, long);

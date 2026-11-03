// [dcl.fct]/1, [over.load]/2: at -fclang-compat=19 a file-scope `extern
// "C"` declaration of a builtin's name differing from the builtin only in
// the return type is reported as an overload differing only in the return
// type (clang 19 checks overloading first; clang 22:
// builtin-redecl-22.cpp), with the builtin-type note (WP-410).
//@ expect: compile-fail
//@ strict-diags
//@ requires: clang<22
//@ diag: 15:16 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 15:16 note "'__builtin_cos' is a builtin with type 'double (double) noexcept'"
//@ diag: 16:18 error "conflicting types for '__builtin_memset'"
//@ diag: 16:18 note "'__builtin_memset' is a builtin with type 'void *(void *, int, unsigned long) noexcept'"
//@ diag: 17:16 error "functions that differ only in their return type cannot be overloaded"
//@ diag: 17:16 note "'__builtin_expect' is a builtin with type 'long (long, long) noexcept'"
extern "C" int __builtin_cos(double);
extern "C" void *__builtin_memset(void *, int, int);
extern "C" int __builtin_expect(long, long);

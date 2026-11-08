// The implicit format(printf, F, A) attribute of __builtin_printf (1, 2),
// __builtin_vprintf (1, 0) and __builtin_vsprintf (2, 0), and the
// signatures of the printf/scanf family builtins (clang 19 Builtins.td:
// the printf ones NoThrow, the scanf ones not; GNU [dcl.attr] extension).
// clang 19 declares no __builtin_fprintf, __builtin_vfprintf,
// __builtin_fscanf or __builtin_vfscanf. A va_list format that is not a
// literal is only -Wformat-nonliteral's (off by default). Verified
// against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:31 warning "format specifies type 'int' but the argument has type 'long'"
//@ diag: 29:34 warning "format specifies type 'char *' but the argument has type 'int'"
//@ diag: 30:25 warning "more '%' conversions than data arguments"
//@ diag: 31:31 warning "data argument not used by format string"
//@ diag: 32:20 warning "format string is not a string literal (potentially insecure)"
//@ diag: 32:20 note "treat the string as an argument to avoid this"
//@ diag: 37:11 error "builtin functions must be directly called"
//@ diag: 38:9 error "use of undeclared identifier '__builtin_fprintf'"
typedef __builtin_va_list va;
static_assert(noexcept(__builtin_printf("%d", 1)));
static_assert(noexcept(__builtin_vprintf("%d", va{})));
static_assert(noexcept(__builtin_vsprintf(nullptr, "%d", va{})));
static_assert(!noexcept(__builtin_scanf("%d", (int *)0)));
static_assert(!noexcept(__builtin_sscanf("", "%d", (int *)0)));
static_assert(!noexcept(__builtin_vscanf("%d", va{})));
static_assert(!noexcept(__builtin_vsscanf("", "%d", va{})));
static_assert(__is_same(decltype(__builtin_printf("")), int));
void f(char *buf, const char *fmt, long l, va ap) {
  __builtin_printf("%d %s\n", l, 3);
  __builtin_printf("%d %d\n", 1);
  __builtin_printf("%d\n", 1, 2);
  __builtin_printf(fmt);
  __builtin_vprintf(fmt, ap);
  __builtin_vsprintf(buf, fmt, ap);
  __builtin_vprintf("%d\n", ap);
}
auto p = &__builtin_printf;
int q = __builtin_fprintf(0, "");

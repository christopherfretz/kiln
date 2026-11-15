// GNU unwinder builtins (an extension; found in unwinder-style code):
// `__builtin_dwarf_cfa`, `__builtin_unwind_init` and
// `__builtin_dwarf_sp_column` take no argument, `__builtin_eh_return` an
// offset converted to size_t and a handler pointer,
// `__builtin_eh_return_data_regno` one integer constant expression
// ([expr.const]; clang `BuiltinConstantArg`). Only
// `__builtin_eh_return_data_regno` is a constant expression
// ([expr.const]/10 with clang `getEHDataRegisterNumber`). WP-1428.
// The same at clang 19 and 22, on x86-64 and AArch64.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 25:25 error "constexpr variable 'sp' must be initialized by a constant expression"
//@ diag: 26:21 error "constexpr variable 'c' must be initialized by a constant expression"
//@ diag: 27:34 error "constexpr variable 'x' must be initialized by a constant expression"
//@ diag: 29:9 error "argument to '__builtin_eh_return_data_regno' must be a constant integer"
//@ diag: 30:40 error "too few arguments to function call, expected 1, have 0"
//@ diag: 31:44 error "too many arguments to function call, expected 1, have 2"
//@ diag: 32:31 error "too many arguments to function call, expected 0, have 1"
//@ diag: 34:25 error "too many arguments to function call, expected 0, have 1"
//@ diag: 35:24 error "too few arguments to function call, expected 2, have 1"
//@ diag: 37:40 error "too many arguments to function call, expected 0, have 1"
//@ diag: 38:39 error "with an lvalue of type 'void *'"
//@ diag: 39:47 error "too many arguments to function call, expected 1, have 2"
//@ diag: 40:27 error "argument to '__builtin_eh_return_data_regno' must be a constant integer"
constexpr unsigned sp = __builtin_dwarf_sp_column();
constexpr void *c = __builtin_dwarf_cfa();
constexpr unsigned long long x = __builtin_extend_pointer(nullptr);
int n;
int a = __builtin_eh_return_data_regno(n);
int b = __builtin_eh_return_data_regno();
int b2 = __builtin_eh_return_data_regno(1, 2);
void *d = __builtin_dwarf_cfa(1);
void f() {
  __builtin_unwind_init(3);
  __builtin_eh_return(1);
}
unsigned e = __builtin_dwarf_sp_column(0);
void g(void *p) { __builtin_eh_return(p, p); }
void h(void *p) { __builtin_extend_pointer(p, p); }
constexpr int k = 2, cr = __builtin_eh_return_data_regno(n + k);
static_assert(__builtin_eh_return_data_regno(k) == -1);

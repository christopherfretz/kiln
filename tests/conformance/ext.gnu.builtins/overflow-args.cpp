// GNU checked arithmetic builtins: argument count and types (clang
// SemaBuiltinOverflow). The operands are integers (bool and unscoped
// enumerations included, scoped enumerations not); the result is a pointer
// to a non-const integer (volatile allowed). The typed forms take their
// fixed parameter types. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 27:30 error "too few arguments to function call, expected 3, have 2"
//@ diag: 28:36 error "too many arguments to function call, expected 3, have 4"
//@ diag: 29:26 error "operand argument to overflow builtin must be an integer type ('float' invalid)"
//@ diag: 30:29 error "operand argument to overflow builtin must be an integer type ('X' invalid)"
//@ diag: 31:26 error "operand argument to overflow builtin must be an integer type ('S' invalid)"
//@ diag: 32:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('const int *' invalid)"
//@ diag: 33:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('int' invalid)"
//@ diag: 34:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('float *' invalid)"
//@ diag: 35:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('S *' invalid)"
//@ diag: 41:33 error "cannot initialize a parameter of type 'int *' with an lvalue of type 'const int *'"
//@ diag: 42:33 error "cannot initialize a parameter of type 'int *' with an rvalue of type 'long *'"
//@ diag: 43:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('std::nullptr_t' invalid)"
//@ diag: 44:32 error "result argument to overflow builtin must be a pointer to a non-const integer type ('int' invalid)"
//@ diag: 45:9 error "builtin functions must be directly called"
enum E { e0 }; enum class S { s0 }; enum Big : long long { b0 };
struct X {};
void f(int a, unsigned u, bool bo, char c, E e, S s, X x, float fl, int *pi,
       const int *cpi, bool *pb, char *pc, E *pe, S *ps, volatile int *vpi,
       float *pf, int &ri, Big *pbig) {
  __builtin_add_overflow(a, a);
  __builtin_add_overflow(a, a, pi, pi);
  __builtin_add_overflow(fl, a, pi);
  __builtin_sub_overflow(a, x, pi);
  __builtin_mul_overflow(s, a, pi);
  __builtin_add_overflow(a, a, cpi);
  __builtin_add_overflow(a, a, a);
  __builtin_add_overflow(a, a, pf);
  __builtin_add_overflow(a, a, ps);
  __builtin_add_overflow(bo, c, pb);
  __builtin_add_overflow(e, u, pe);
  __builtin_add_overflow(a, a, vpi);
  __builtin_add_overflow(a, a, &ri);
  __builtin_mul_overflow(a, a, pbig);
  __builtin_sadd_overflow(a, a, cpi);
  __builtin_sadd_overflow(a, a, (long *)0);
  __builtin_add_overflow(a, a, nullptr);
  __builtin_add_overflow(a, a, 0);
  (void)__builtin_add_overflow;
}

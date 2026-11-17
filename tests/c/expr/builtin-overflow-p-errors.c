// GNU __builtin_*_overflow_p: the operands are of integer type (bool and
// enumerations included), the third argument of an integer type other
// than bool or an enumeration, and there are exactly three arguments.
// Wording follows clang's for the overflow builtins. Compared with GCC
// 16.1, which rejects the same calls (with its own wording). A third
// argument with side effects or a call is evaluated, so the call is then
// not an integer constant expression. WP-1549.
//@ std: c17
//@ expect: compile-fail
//@ oracle: gcc
//@ skip(clang): clang 19/22 do not implement __builtin_*_overflow_p
//@ strict-diags
//@ diag: 27:34 error "operand argument to overflow builtin must be an integer type ('double' invalid)"
//@ diag: 28:37 error "operand argument to overflow builtin must be an integer type ('int *' invalid)"
//@ diag: 29:40 error "result argument to overflow builtin must be an integer type other than 'bool' or an enumeration ('double' invalid)"
//@ diag: 30:40 error "result argument to overflow builtin must be an integer type other than 'bool' or an enumeration ('_Bool' invalid)"
//@ diag: 31:40 error "result argument to overflow builtin must be an integer type other than 'bool' or an enumeration ('enum E' invalid)"
//@ diag: 32:40 error "result argument to overflow builtin must be an integer type other than 'bool' or an enumeration ('int *' invalid)"
//@ diag: 33:38 error "too few arguments to function call, expected 3, have 2"
//@ diag: 34:43 error "too many arguments to function call, expected 3, have 4"
//@ diag: 36:47 error "static assertion expression is not an integral constant expression"
//@ diag: 37:47 error "static assertion expression is not an integral constant expression"
enum E { A };
int p;
int ok1 = __builtin_add_overflow_p((_Bool)1, 1, 0);
int ok2 = __builtin_add_overflow_p((enum E)0, 1, 0);
int c = __builtin_add_overflow_p(1.0, 1, 0);
int d = __builtin_sub_overflow_p(1, &p, 0);
int e = __builtin_mul_overflow_p(1, 1, 1.0);
int g = __builtin_add_overflow_p(1, 1, (_Bool)0);
int h = __builtin_add_overflow_p(1, 1, (enum E)0);
int i = __builtin_add_overflow_p(1, 1, &p);
int j = __builtin_add_overflow_p(1, 1);
int k = __builtin_add_overflow_p(1, 1, 0, 0);
int f(void);
_Static_assert(__builtin_add_overflow_p(1, 2, f()) == 0, "");
_Static_assert(__builtin_add_overflow_p(1, 2, p = 1) == 0, "");

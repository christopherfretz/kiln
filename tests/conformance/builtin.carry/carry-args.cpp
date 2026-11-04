// The carry builtins take fixed parameter types (clang Builtins.td
// "UcUcUcUcUc*" .. "ULLiULLiULLiULLiULLi*"; not custom-checked): the
// arguments convert as to an ordinary function's parameters, so a carry-out
// pointer of another type, a missing argument or a non-integer operand that
// does not convert is ill-formed. Verified against clang 19.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 15:29 error "cannot initialize a parameter of type 'unsigned long long *' with an rvalue of type 'unsigned int *'"
//@ diag: 16:28 error "cannot initialize a parameter of type 'unsigned char *' with an rvalue of type 'int *'"
//@ diag: 17:25 error "too few arguments to function call, expected 4, have 3"
//@ diag: 18:18 error "no viable conversion from 'S' to 'unsigned int'"
//@ diag: 19:28 error "cannot initialize a parameter of type 'unsigned long *' with an lvalue of type 'const unsigned long *'"
struct S {};
void f(unsigned u, int i, S s, const unsigned long *cl) {
  __builtin_addcll(1, 2, 0, &u);
  __builtin_subcb(1, 2, 0, &i);
  __builtin_subc(1, 2, 3);
  __builtin_addc(s, 2, 0, &u);
  __builtin_subcl(1, 2, 0, cl);
}

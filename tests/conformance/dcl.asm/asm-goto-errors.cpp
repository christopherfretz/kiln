// [dcl.asm] (GNU extension): `asm goto` needs its label list; labels
// must be declared in the function, their names share the operand
// namespace, and they are numbered after the operands. Plain asm takes no
// label list.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 17:25 error "expected ':'"
//@ diag: 20:23 error "use of undeclared label 'nowhere'"
//@ diag: 23:33 error "duplicate use of asm operand name \"a\""
//@ diag: 23:26 note "asm operand name \"a\" first referenced here"
//@ diag: 27:21 error "expected identifier"
//@ diag: 30:15 error "invalid operand number in inline asm string"
//@ diag: 34:14 error "expected ')'"
//@ diag: 38:16 error "unknown symbolic operand name in inline assembly string"
void f1(int x) {
  asm goto("" : : "r"(x));
}
void f2() {
  asm goto("" : : : : nowhere);
}
void f3(int x) {
  asm goto("" : : [a]"r"(x) : : a);
a:;
}
void f4() {
  asm goto("" ::: : 3);
}
void f5() {
  asm goto("%l2" ::: : l);
l:;
}
void f6() {
  asm("" ::: : l);
l:;
}
void f7() {
  asm goto("%l[m]" :::: l);
l:;
}

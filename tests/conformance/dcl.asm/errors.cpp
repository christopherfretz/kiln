// [dcl.asm] (GNU extended asm): constraint, operand, template and clobber
// checks (clang's ActOnGCCAsmStmt), on AArch64 for its register names and
// the operand-width warning.
//@ expect: compile-fail
//@ args: --target=aarch64-unknown-linux-gnu
//@ strict-diags
//@ diag: 32:12 error "invalid output constraint 'r' in asm"
//@ diag: 33:17 error "invalid lvalue in asm output"
//@ diag: 34:17 error "invalid lvalue in asm output"
//@ diag: 35:13 error "invalid input constraint '=r' in asm"
//@ diag: 36:17 error "invalid lvalue in asm input for constraint 'm'"
//@ diag: 37:14 error "unknown register name 'foo' in asm"
//@ diag: 38:9 error "invalid operand number in inline asm string"
//@ diag: 39:10 error "unknown symbolic operand name in inline assembly string"
//@ diag: 40:8 error "invalid % escape in inline assembly string"
//@ diag: 41:28 error "asm constraint has an unexpected number of alternatives: 2 vs 1"
//@ diag: 42:34 error "duplicate use of asm operand name \"x\""
//@ diag: 42:21 note "asm operand name \"x\" first referenced here"
//@ diag: 44:22 error "invalid input constraint '1' in asm"
//@ diag: 46:12 error "invalid output constraint '+&m' in asm"
//@ diag: 47:17 error "dereference of pointer to incomplete type 'I'"
//@ diag: 29:8 note "forward declaration of 'I'"
//@ diag: 48:17 error "invalid type 'void' in asm input for constraint 'r'"
//@ diag: 49:27 warning "value size does not match register size specified by the constraint and modifier"
//@ diag: 49:12 note "use constraint modifier \"w\""
//@ diag: 49:36 warning "value size does not match register size specified by the constraint and modifier"
//@ diag: 49:16 note "use constraint modifier \"w\""
//@ diag: 53:10 error "cannot use wide string literal in 'asm'"
struct I;
void f(int a, const int c, I* ip) {
  int r; long l; char ch;
  asm("" : "r"(r));
  asm("" : "=r"(a + 1));
  asm("" : "=r"(c));
  asm("" :: "=r"(a));
  asm("" :: "m"(a + 1));
  asm("" ::: "foo");
  asm("%3" : "=r"(r) : "r"(a));
  asm("%[x]" : "=r"(r) : "r"(a));
  asm("%" : "=r"(r));
  asm("" : "=r,m"(r) : "r"(a));
  asm("" : [x] "=r"(r) : [x] "r"(a));
  asm("" : "=r"(r) : "0"(l));
  asm("" : "=r"(r) : "1"(a));
  asm("" : "=&m"(r));
  asm("" : "+&m"(r));
  asm("" : "=r"(*ip));
  asm("" :: "r"(f(1,2,0)));
  asm("mov %0, %1" : "=r"(r) : "r"(a));
  asm("" :: "i"(a));
  asm("" :: "n"(a));
  asm("" : : "x"(a));
  asm(L"");
  asm("" : "=r"(r) : "r"(a) : "x0", "w1", "x29", "fp", "lr", "r3", "sp", "v1", "d0");
  asm("" : "=r"(r) : "r"(a) : "unwind");
}

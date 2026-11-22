// [expr.new]/8, /22, [expr.ass]: an array new-expression with a bound
// known only at run time, as the operand of an assignment, a
// conditional and a comma (the full-expressions -Winteger-overflow
// evaluates, which crashed on the element constructor), default- and
// value-initializes its elements. Reduced from LLVM 19 (`DeclObjC.cpp`).
//@ expect: run
//@ stdout: 0 0 7 7 0
extern "C" int printf(const char *, ...);
struct S { unsigned x = 0; };
struct C { C() : v(7) {} int v; };
S *p;
C *pc;
unsigned *pu;
void make(unsigned long n) {
  p = new S[n];
  pc = n ? new C[n] : nullptr;
  pu = nullptr, pu = new unsigned[n]();
}
int main() {
  make(3);
  printf("%u %u %d %d %u\n", p[0].x, p[2].x, pc[0].v, pc[2].v, pu[1]);
  delete[] p;
  delete[] pc;
  delete[] pu;
}

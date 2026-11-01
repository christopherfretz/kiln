// [dcl.asm] (GNU extension): `asm goto` lists the labels its template may
// transfer control to; an empty template falls through.
//@ expect: run
//@ xfail(kiln): 'asm goto' (callbr lowering, label operands) is not implemented yet
int f() {
  asm goto("" : : : : taken);
  return 0;
taken:
  return 1;
}
int main() { return f(); }

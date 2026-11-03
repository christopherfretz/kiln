// [expr.sub]/2, [expr.add]/4, [expr.pre.incr], [expr.ref]/7: the address
// arithmetic of clang 20+ at -fclang-compat=22 (spec/codegen.md §3 "GEP
// flags"): `getelementptr inbounds nuw` for member access and for array
// subscripts and pointer additions with an unsigned index or a positive
// constant step; plain `inbounds` for signed indices, subtraction and
// decrement. Compared with clang 22's IR exactly.
//@ expect: ir
// clang 22 has no C++26 erroneous values (P2795R5, spec/codegen.md §7q).
//@ args: -ftrivial-auto-var-init=uninitialized
//@ requires: clang>=22
//@ requires: llvm>=22
struct Inner { int x, y; };
struct Outer { char c; Inner in; int arr[4]; };

int member(Outer *o) { return o->in.y + o->arr[1]; }
int subscript(int *p, unsigned u, int s, unsigned long ul) {
  return p[u] + p[s] + p[ul] + u[p];
}
int *arith(int *p, unsigned u, int s) { return (p + u) - s + (s - 1) - u; }
int steps(int *p) {
  int a = *p++;
  int b = *++p;
  int c = *p--;
  int d = *--p;
  return a + b + c + d;
}
int local_array(unsigned i, int j) {
  int a[8];
  for (unsigned k = 0; k < 8; ++k)
    a[k] = 0;
  a[i] = 1;
  return a[j] + a[3];
}

// [cpp.pragma]/1 (implementation-defined pragmas), [stmt.iter], [dcl.attr]:
// clang's loop-hint pragmas. A misplaced hint (at namespace scope, in a
// class, before a non-loop statement or the end of a block), an unknown
// option, a bad argument or value, a non-constant count, an incompatible or
// duplicate pair, and extra tokens after `#pragma unroll N` are diagnosed
// as clang does (clang ParsePragmaLoopHint, handleLoopHintAttr,
// CheckForIncompatibleAttributes). `GCC ivdep` is an ignored pragma.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 41:1 error "expected unqualified-id"
//@ diag: 44:1 error "this pragma cannot appear in struct declaration"
//@ diag: 49:3 error "expected a for, while, or do-while loop to follow '#pragma c"
//@ diag: 52:20 error "invalid option 'bogus'; expected vectorize, vectorize_width,"
//@ diag: 51:3 error "expected a for, while, or do-while loop to follow '#pragma u"
//@ diag: 54:27 error "invalid argument; expected 'enable', 'full' or 'disable'"
//@ diag: 56:36 error "invalid value '0'; must be positive"
//@ diag: 58:33 error "invalid value '-1'; must be positive"
//@ diag: 60:15 error "incompatible directives 'unroll(full)' and 'unroll_count(2)'"
//@ diag: 63:15 error "duplicate directives 'unroll(full)' and 'unroll(disable)'"
//@ diag: 65:33 error "expression is not an integral constant expression"
//@ diag: 65:33 note "function parameter 'n' with unknown value cannot be used in "
//@ diag: 47:20 note "declared here"
//@ diag: 67:19 error "missing option; expected vectorize, vectorize_width, interle"
//@ diag: 69:26 error "expected '('"
//@ diag: 71:33 error "invalid option 'junk'; expected vectorize, vectorize_width, "
//@ diag: 73:18 warning "extra tokens at end of '#pragma nounroll' - ignored"
//@ diag: 75:18 warning "extra tokens at end of '#pragma unroll' - ignored"
//@ diag: 77:29 error "invalid argument; expected 'disable'"
//@ diag: 79:30 error "invalid argument; expected 'enable', 'assume_safety' or 'dis"
//@ diag: 81:28 error "missing argument; expected 'enable', 'full' or 'disable'"
//@ diag: 83:39 error "vectorize_width loop hint malformed; use vectorize_width(X, "
//@ diag: 85:16 error "invalid argument of type 'double'; expected an integer type"
//@ diag: 95:3 error "expected a for, while, or do-while loop to follow '#pragma u"
//@ diag: 97:1 error "expected statement"
//@ diag: 98:1 error "expected unqualified-id"
//@ diag: 100:9 error "use of undeclared identifier 'x'"
//@ diag: 102:1 error "this pragma cannot appear in struct declaration"
//@ diag: 108:20 error "invalid option 'bogus'; expected vectorize, vectorize_width,"
//@ diag: 109:1 error "expected statement"
#pragma unroll
int x0;
struct S0 {
#pragma clang loop unroll(full)
  int y;
};
void f(int *a, int n) {
#pragma clang loop unroll(full)
  a[0] = 1;
#pragma unroll 4
  int z = 0;
#pragma clang loop bogus(1)
  for (;;) {}
#pragma clang loop unroll(maybe)
  for (;;) {}
#pragma clang loop vectorize_width(0)
  for (;;) {}
#pragma clang loop unroll_count(-1)
  for (;;) {}
#pragma clang loop unroll(full) unroll_count(2)
  for (;;) {}
#pragma clang loop unroll(full)
#pragma clang loop unroll(disable)
  for (;;) {}
#pragma clang loop unroll_count(n)
  for (;;) {}
#pragma clang loop
  for (;;) {}
#pragma clang loop unroll
  for (;;) {}
#pragma clang loop unroll(full) junk
  for (;;) {}
#pragma nounroll 3
  for (;;) {}
#pragma unroll 4 5
  for (;;) {}
#pragma clang loop pipeline(enable)
  for (;;) {}
#pragma clang loop vectorize(full)
  for (;;) {}
#pragma clang loop unroll()
  for (;;) {}
#pragma clang loop vectorize_width(4, bogus)
  for (;;) {}
#pragma unroll 1.5
  for (;;) {}
#pragma GCC ivdep
  for (;;) {}
  if (n)
#pragma unroll
    while (n) {}
#pragma unroll
  [[likely]] for (;;) {}
#pragma unroll
  {}
#pragma unroll
}
#pragma unroll
int x;
int y = x;
struct S {
#pragma unroll
  int a;
  int b = a;
};
void h() {
#pragma unroll
#pragma clang loop bogus
}

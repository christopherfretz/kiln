// -Wempty-body (clang 19 DiagnoseEmptyStmtBody / DiagnoseEmptyLoopBody,
// WP-191): a `;` body on the line of the `)`, unless a macro expanding to
// nothing precedes it. `if` (no `else`, not consteval), `switch` and
// range-based `for` warn at once; `for` and `while` only when the next
// statement of the block is a compound statement or is indented further.
// Template definitions warn, instantiations do not.
//@ expect: compile-ok
//@ strict-diags
//@ diag: 40:9 warning "if statement has empty body"
//@ diag: 40:9 note "put the semicolon on a separate line to silence this warning"
//@ diag: 44:11 warning "if statement has empty body"
//@ diag: 44:11 note "put the semicolon on a separate line to silence this warning"
//@ diag: 48:18 warning "if statement has empty body"
//@ diag: 48:18 note "put the semicolon on a separate line to silence this warning"
//@ diag: 50:22 warning "if statement has empty body"
//@ diag: 50:22 note "put the semicolon on a separate line to silence this warning"
//@ diag: 54:13 warning "switch statement has empty body"
//@ diag: 54:13 note "put the semicolon on a separate line to silence this warning"
//@ diag: 56:18 warning "range-based for loop has empty body"
//@ diag: 56:18 note "put the semicolon on a separate line to silence this warning"
//@ diag: 59:12 warning "while loop has empty body"
//@ diag: 59:12 note "put the semicolon on a separate line to silence this warning"
//@ diag: 65:11 warning "for loop has empty body"
//@ diag: 65:11 note "put the semicolon on a separate line to silence this warning"
//@ diag: 69:30 warning "for loop has empty body"
//@ diag: 69:30 note "put the semicolon on a separate line to silence this warning"
//@ diag: 71:20 warning "for loop has empty body"
//@ diag: 71:20 note "put the semicolon on a separate line to silence this warning"
//@ diag: 79:13 warning "while loop has empty body"
//@ diag: 79:13 note "put the semicolon on a separate line to silence this warning"
//@ diag: 83:9 warning "if statement has empty body"
//@ diag: 83:9 note "put the semicolon on a separate line to silence this warning"
//@ diag: 84:12 warning "while loop has empty body"
//@ diag: 84:12 note "put the semicolon on a separate line to silence this warning"
#define EMPTY
#define CALL(x)
#define SEMI ;
int g(int);
void f(int x, int (&a)[3]) {
  if (x);
  if (x)
    ;
  if (x); else g(1);
  if ((x));
  if (x) EMPTY;
  if (x) CALL(0);
  if (x) SEMI
  if (x) /* c */ ;
  if (x) /* c */ EMPTY /* d */;
  if constexpr (true);
  if consteval {} else {}
  if (x) {
  } else;
  switch (x);
  switch (x) EMPTY;
  for (int v : a);
  g(2);
  while (x);
  while (x);
    g(3);
  while (x) EMPTY;
    g(4);
  while (x);
  g(5);
  for (;;);
  {
    g(6);
  }
  for (int i = 0; i < x; ++i);
    g(x);
  for (;;) /* c */ ;
  {}
  for (;;);
  label: g(7);
  {
    while (x);
  }
  do ; while (x);
  while (x) ;
  {}
}
template <class T> void t(T x) {
  if (x);
  while (x);
  { }
}
template void t<int>(int);

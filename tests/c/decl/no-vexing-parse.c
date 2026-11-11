// C23 6.7.7.4 (function declarators): a block-scope `int g();` declares a
// function in C as in C++, but C has no direct-initialization syntax for
// it to be mistaken for, so there is no -Wvexing-parse
// (clang `warnAboutAmbiguousFunction` runs only in C++).
//@ expect: compile-ok
//@ strict-diags
int f(void) {
  int g();
  int (h)();
  struct S { int a; } s();
  return 0;
}

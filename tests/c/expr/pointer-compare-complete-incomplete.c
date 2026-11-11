// C99 6.5.8p2 (C11 dropped it): an ordered comparison of pointers to
// compatible types, one complete and one incomplete, is a pedantic
// -Wc11-extensions warning before C11 (clang
// `ext_typecheck_compare_complete_incomplete_pointers`); equality and
// incompatible pointee types are not (WP-1312, WP-1309 follow-up).
//@ std: c99
//@ args: -pedantic
//@ strict-diags
//@ diag: 15:12 warning "pointer comparisons before C11 need to be between two complete or two incomplete types; 'int (*)[]' is incomplete and 'int (*)[3]' is complete"
//@ diag: 16:12 warning "pointer comparisons before C11 need to be between two complete or two incomplete types; 'int (*)[3]' is complete and 'int (*)[]' is incomplete"
//@ diag: 18:12 warning "comparison of distinct pointer types ('void *' and 'int *')"
//@ diag: 19:12 warning "self-comparison always evaluates to false"
struct S;
void f(int (*p)[], int (*q)[3], void *v, int *i, struct S *s) {
  (void)(p < q);
  (void)(q >= p);
  (void)(p == q);
  (void)(v < i);
  (void)(s < s);
}

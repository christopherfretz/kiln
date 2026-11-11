// C17 6.5.8p2, 6.5.9p2: comparing pointers to incompatible types, a pointer
// with a non-null integer, or (ordered) a pointer with a null pointer constant
// or two function pointers is a constraint violation, which clang diagnoses
// with a warning in C and evaluates on the converted pointers
// (-Wcompare-distinct-pointer-types, -Wpointer-integer-compare,
// -Wordered-compare-function-pointers; clang `CheckCompareOperands`). A
// function pointer against a null pointer constant, including `(void *)0`, is
// valid (6.3.2.3p3). WP-1285.
//@ std: c17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 31:11 warning "comparison of distinct pointer types ('int *' and 'long *')"
//@ diag: 32:11 warning "comparison of distinct pointer types ('int *' and 'long *')"
//@ diag: 36:11 warning "comparison of distinct pointer types ('int *' and 'void *')"
//@ diag: 37:11 warning "comparison between pointer and integer ('int *' and 'int')"
//@ diag: 38:10 warning "comparison between pointer and integer ('int' and 'int *')"
//@ diag: 39:11 warning "ordered comparison between pointer and integer ('int *' and 'long')"
//@ diag: 40:11 warning "comparison between pointer and integer ('int *' and 'int')"
//@ diag: 41:11 warning "comparison between pointer and integer ('int *' and 'unsigned char')"
//@ diag: 42:10 warning "self-comparison always evaluates to false"
//@ diag: 42:10 warning "ordered comparison of function pointers ('kfn' (aka 'int (*)(int)') and 'kfn')"
//@ diag: 44:10 warning "comparison of distinct pointer types ('kfn' (aka 'int (*)(int)') and 'int *')"
typedef int (*kfn)(int);
struct S;
int g(kfn k, void *v, int *ip, long *lp, const int *cip, struct S *sp, int i,
      long l, unsigned char uc) {
  int r = 0;
  r += k == (void *)0;
  r += k == 0;
  r += (void *)0 == k;
  r += ip == lp;
  r += ip < lp;
  r += ip == cip;
  r += ip < cip;
  r += ip == v;
  r += ip < v;
  r += ip == i;
  r += i != ip;
  r += ip < l;
  r += ip == 1;
  r += ip == uc;
  r += k < k;
  r += sp < (struct S *)v;
  r += k == ip;
  r += ip == 0L;
  return r;
}

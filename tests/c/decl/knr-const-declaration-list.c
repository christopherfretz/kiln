// C17 6.9.1p6 (function definitions): in a K&R definition, a `const`
// after the identifier list starts the declaration list. C has no
// function cv-qualifiers, so it is not a qualifier on the declarator
// (gcc.c-torture compile/20040309-1.c, compile/980825-1.c, execute/930513-2.c).
//@ expect: run
//@ std: c89
//@ strict-diags
//@ stdout: 6 2 4
//@ diag: 14:5 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
//@ diag: 19:1 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
//@ diag: 24:12 warning "a function definition without a prototype is deprecated in all versions of C and is not supported in C23"
int printf(const char *, ...);

int first(x) const char *x;
{
  return *x - '0';
}

second(x) const char *x;
{
  return x[1] - '0';
}

static int sum(a, b) const int *a; const int *b;
{
  return *a + *b;
}

int main(void) {
  int p = 1, q = 3;
  printf("%d %d %d\n", first("6"), second("12"), sum(&p, &q));
  return 0;
}

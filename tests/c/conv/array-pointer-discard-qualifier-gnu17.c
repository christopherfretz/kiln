// C17 6.5.16.1p1 (C23 6.7.3p9): a pointer to an array of const (volatile)
// T converted to a pointer to an array of T drops a qualifier of the
// pointee, the array's qualifiers being its element's: clang's
// "discards qualifiers" warning (-Wincompatible-pointer-types-discards-
// qualifiers, nested pointers included), not "incompatible pointer
// types", in every context and for typedef'd, multi-dimensional and VLA
// spellings. WP-1427.
//@ args: -std=gnu17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 22:38 warning "returning 'const Row *' (aka 'const int (*)[3]') from a function with result type 'int (*)[3]' discards qualifiers"
//@ diag: 25:9 warning "initializing 'int (*)[3]' with an expression of type 'const int (*)[3]' discards qualifiers"
//@ diag: 26:5 warning "assigning to 'int (*)[3]' from 'const int (*)[3]' discards qualifiers"
//@ diag: 27:8 warning "passing 'const int (*)[3]' to parameter of type 'int (*)[3]' discards qualifiers"
//@ diag: 21:17 note "passing argument to parameter 'p' here"
//@ diag: 28:9 warning "initializing 'int (*)[2][3]' with an expression of type 'const int (*)[2][3]' discards qualifiers"
//@ diag: 29:9 warning "initializing 'int (*)[3]' with an expression of type 'volatile int (*)[3]' discards qualifiers"
//@ diag: 30:16 warning "initializing 'const int (**)[3]' with an expression of type 'int (**)[3]' discards qualifiers in nested pointer types"
//@ diag: 31:9 warning "initializing 'int (*)[n]' with an expression of type 'const int (*)[n]' discards qualifiers"
typedef int Row[3];
void take(int (*p)[3]);
int (*ret(const Row *p))[3] { return p; }
void g(const int (*ca)[3], const int (*cm)[2][3], volatile int (*va)[3],
       int (**pp)[3], int n, const int (*cv)[n]) {
  int (*p)[3] = ca;
  p = ca;
  take(ca);
  int (*s)[2][3] = cm;
  int (*t)[3] = va;
  const int (**u)[3] = pp;
  int (*w)[n] = cv;
  (void)p; (void)s; (void)t; (void)u; (void)w;
}

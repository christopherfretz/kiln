// C17 6.5.16.1p1, 6.7.6.2p6: pointers to arrays of different bounds or of
// incompatible element types stay incompatible whatever their
// qualifiers: clang 19 warns -Wincompatible-pointer-types (an error from
// clang 22, array-pointer-incompatible-22.c). WP-1427.
//@ requires: clang<22
//@ args: -std=gnu17
//@ expect: compile-ok
//@ strict-diags
//@ diag: 13:15 warning "incompatible pointer types initializing 'const int (*)[3]' with an expression of type 'int (*)[4]'"
//@ diag: 14:15 warning "incompatible pointer types initializing 'const int (*)[3]' with an expression of type 'long (*)[3]'"
//@ diag: 15:15 warning "incompatible pointer types initializing 'const int (*)[3]' with an expression of type 'unsigned int (*)[3]'"
void g(int (*a)[4], long (*l)[3], unsigned (*u)[3]) {
  const int (*q)[3] = a;
  const int (*r)[3] = l;
  const int (*s)[3] = u;
  (void)q; (void)r; (void)s;
}

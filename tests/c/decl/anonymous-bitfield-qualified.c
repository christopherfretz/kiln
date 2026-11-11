// C23 6.7.3.2p13 (structure and union specifiers): an unnamed bit-field
// may be qualified in C (clang accepts it silently); C++ forbids a
// cv-qualified anonymous bit-field ([class.bit]p2).
// gcc.c-torture: compile/pr48517.c.
//@ expect: compile-ok
//@ args: -pedantic
//@ strict-diags
struct S { const int : 3; volatile unsigned : 0; int x; };
int s = sizeof(struct S);

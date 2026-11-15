// A constant offsetof index of unsigned type (unsigned char, _Bool,
// unsigned) with the top bit of its own width set counts at its true
// value, zero-extended, as GCC does, and equals the run-time value for
// the same index (found building an open-source RPC library).
//@ skip(clang): clang 19 and 22 sign-extend an unsigned constant offsetof index from its own width
//@ expect: run
//@ args: -std=gnu17
//@ stdout: 204 204 5 5 3000000004 3000000004
#include <stddef.h>
#include <stdio.h>
struct C { int n; char a[300]; };
struct Big { int n; char a[4000000000u]; };
_Static_assert(offsetof(struct C, a[(unsigned char)200]) == 204, "");
_Static_assert(offsetof(struct C, a[(_Bool)1]) == 5, "");
_Static_assert(offsetof(struct Big, a[3000000000u]) == 3000000004u, "");
static size_t rc(unsigned char i) { return offsetof(struct C, a[i]); }
static size_t rb(_Bool b) { return offsetof(struct C, a[b]); }
static size_t ru(unsigned i) { return offsetof(struct Big, a[i]); }
int main(void) {
  volatile unsigned char c = 200;
  volatile _Bool b = 1;
  volatile unsigned u = 3000000000u;
  printf("%zu %zu %zu %zu %zu %zu\n", offsetof(struct C, a[(unsigned char)200]), rc(c),
         offsetof(struct C, a[(_Bool)1]), rb(b), offsetof(struct Big, a[3000000000u]), ru(u));
  return 0;
}

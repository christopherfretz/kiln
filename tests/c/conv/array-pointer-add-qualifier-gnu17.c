// C23 6.7.3p9 (N2607): an array type and its element type are identically
// qualified, so `T (*)[N]` converts to `const T (*)[N]` as any pointer
// gains a qualifier on its pointee (C17 6.5.16.1p1): initialization,
// assignment, argument passing, return, equality comparison (6.5.9p2);
// multi-dimensional, volatile, typedef'd, VLA and `void *` targets.
// Before C23 6.7.3p9 (C17) applied the qualifier to the element only,
// making this a constraint violation; clang 19 and 22 accept it silently
// there too (their canonical array type carries its element's
// qualifiers), GCC 16.1 warns under -pedantic. kiln follows clang in
// every C mode, and warns as GCC does only under -pedantic
// (array-pointer-qualifiers-pedantic-gnu17.c, WP-1435). Found building an
// open-source image codec. WP-1427.
//@ args: -std=gnu17 -Wall
//@ expect: compile-ok
//@ strict-diags
typedef unsigned int Buckets[256];
unsigned int sum(Buckets *h) { const Buckets *const c = h; return (*c)[0]; }

void take(const int (*p)[3]);
const int (*ret(int (*p)[3]))[3] { return p; }
const Buckets *ret_typedef(Buckets *h) { return h; }

int f(int n, int (*a)[3], int (*m)[2][3], volatile int (*vq)[3], int (*v)[n]) {
  const int (*ca)[3] = a;
  ca = a;
  take(a);
  const int (*cm)[2][3] = m;
  const volatile int (*cvq)[3] = vq;
  volatile int (*va)[3] = a;
  const int (*cv)[n] = v;
  const int (*cvn)[3] = (int (*)[3])v;
  const void *pv = a;
  _Bool same = a == ca;
  return (*ca)[0] + (*cm)[1][2] + (*cvq)[0] + (*va)[1] + (*cv)[0] + (*cvn)[0] + same + (pv != 0);
}

typedef const int CRow[3];
void take_row(CRow *r);
void g(int (*a)[3]) { CRow *r = a; take_row(a); take_row(r); }

//@ strict-diags
//@ args: -Wno-vla-cxx-extension
//@ diag: 10:7 warning "variable length array folded to constant array as an extension"
//@ diag: 11:8 warning "variable length array folded to constant array as an extension"
//@ diag: 13:14 warning "variable length array folded to constant array as an extension"
// WP-165: a folded bound of a member array or a namespace-scope typedef.
const _Atomic(unsigned) n = 2;
const float f = 5.5f;
struct S {
  int a[n];
  char b[(int)f];
};
typedef long T[(int)f + n];
static_assert(sizeof(S::a) == 2 * sizeof(int));
static_assert(sizeof(S::b) == 5);
static_assert(sizeof(T) == 7 * sizeof(long));

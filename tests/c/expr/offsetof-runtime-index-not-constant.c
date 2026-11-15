// An offsetof whose array index is not a constant is not an integer
// constant expression (C17 6.6p6), nor an arithmetic constant
// expression for a static initializer (6.6p8, 6.7.9p4): clang's errors
// at the index, or at the whole offsetof for an initializer. At block
// scope it makes an array a VLA (6.7.6.2p4). clang places an integer
// constant expression error at the first operand that is not one (the
// index) and gives no notes; kiln places it at the expression's start
// with the evaluation's notes (a general C gap), so those three lines
// carry no column and the list is not strict.
//@ expect: compile-fail
//@ args: -std=gnu17
//@ diag: 21 error "static assertion expression is not an integral constant expression"
//@ diag: 22:5 error "variable length array declaration not allowed at file scope"
//@ diag: 23:19 error "initializer element is not a compile-time constant"
//@ diag: 24 error "expression is not an integer constant expression"
//@ diag: 28 error "expression is not an integer constant expression"
//@ diag: 31:21 error "initializer element is not a compile-time constant"
#include <stddef.h>
struct flex { int n; long data[]; };
int g;
_Static_assert(offsetof(struct flex, data[g]) == 8, "");
int arr[offsetof(struct flex, data[g])];
static size_t s = offsetof(struct flex, data[g]);
enum { E = offsetof(struct flex, data[g]) };
size_t f(int n) {
  int v[offsetof(struct flex, data[n])];
  switch (n) {
  case offsetof(struct flex, data[n]):
    break;
  }
  static size_t t = offsetof(struct flex, data[n]);
  return sizeof v + t;
}

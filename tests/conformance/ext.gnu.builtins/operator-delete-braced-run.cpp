// `__builtin_operator_new` / `__builtin_operator_delete` resolve among
// the global allocation functions as a call does, so a braced-init-list
// argument copy-list-initializes its parameter ([over.ics.list],
// [expr.call]/7; clang warns about the braces around a scalar)
// (WP-1745).
//@ expect: run
//@ stdout: ok 42
//@ diag: 15:36 warning "braces around scalar initializer"
//@ diag: 18:29 warning "braces around scalar initializer"
#include <cstdio>
#include <cstddef>

int main() {
  std::size_t n = sizeof(int);
  void* m = __builtin_operator_new({n});
  *static_cast<int*>(m) = 42;
  std::printf("ok %d\n", *static_cast<int*>(m));
  __builtin_operator_delete({m});
}

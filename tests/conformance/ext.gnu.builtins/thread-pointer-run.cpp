// WP-782: `__builtin_thread_pointer()` (clang, every target kiln supports)
// returns the calling thread's thread pointer, `void *`. Two calls in one
// thread agree.
//@ expect: run
//@ stdout: 1 1
#include <cstdio>

static_assert(__has_builtin(__builtin_thread_pointer));

int main() {
  void *a = __builtin_thread_pointer();
  void *b = __builtin_thread_pointer();
  std::printf("%d %d\n", a == b, a != nullptr);
}

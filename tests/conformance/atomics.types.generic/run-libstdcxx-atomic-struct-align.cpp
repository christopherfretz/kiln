// [atomics.types.generic] libstdc++'s std::atomic<T> passes
// std::__addressof(_M_i) to the generic __atomic_load/__atomic_store;
// _M_i is declared alignas(sizeof(T)) for a power-of-two size, so for an
// 8-byte struct of two ints the access is a naturally aligned (lock-free,
// inline) 8-byte atomic. Clang's EmitPointerWithAlignment takes the
// alignment of the std::__addressof operand lvalue (WP-819); with the
// pointee type's (4) LLVM lowers the access to a libatomic call and the
// link fails (run tests never link -latomic).
//@ expect: run
//@ args: -stdlib=libstdc++
//@ strict-diags
//@ stdout: 2 4
#include <atomic>
#include <cstdio>
struct T { int a; int b; };
int main() {
  std::atomic<T> t{T{1, 2}};
  int x = t.load().b;
  t.store(T{3, 4});
  std::printf("%d %d\n", x, t.load().b);
}

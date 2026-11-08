// libstdc++'s std::atomic<T> passes std::__addressof(_M_i) to the
// generic __atomic_load/__atomic_store; _M_i is declared
// alignas(sizeof(T)) for a power-of-two size, so for an 8-byte struct of
// two ints the access is a naturally aligned (lock-free, inline) 8-byte
// atomic. Clang sees the member's alignment through std::__addressof (a
// library builtin); kiln uses the pointee type's (4), so LLVM lowers the
// under-aligned atomic access to a libatomic call and the link fails.
//@ expect: run
//@ args: -stdlib=libstdc++
//@ strict-diags
//@ stdout: 2 4
//@ xfail(kiln): WP-774 follow-up, the alignment of std::__addressof(x) is the pointee type's, not x's (generic __atomic_load/__atomic_store under-aligned, libatomic call)
#include <atomic>
#include <cstdio>
struct T { int a; int b; };
int main() {
  std::atomic<T> t{T{1, 2}};
  int x = t.load().b;
  t.store(T{3, 4});
  std::printf("%d %d\n", x, t.load().b);
}

// __atomic_is_lock_free with an integer cast to a pointer as its object
// argument: the integer's alignment counts (clang 19 folds the argument
// first), as in libstdc++'s std::atomic<T>::is_lock_free, which passes
// reinterpret_cast<void *>(-_S_alignment) and needs no libatomic.
//@ std: c++20
//@ expect: run
//@ stdout: 1 1
#include <atomic>
#include <cstdio>
struct S { int a, b; };
static_assert(__atomic_is_lock_free(8, (void *)-8));
static_assert(__atomic_is_lock_free(8, reinterpret_cast<char *>(16)));
static_assert(__atomic_always_lock_free(4, (void *)-4));
static_assert(!__atomic_always_lock_free(8, (void *)-4));
static_assert(!__atomic_always_lock_free(8, (void *)4));
constexpr long k = -8;
static_assert(__atomic_is_lock_free(sizeof(long), reinterpret_cast<void *>(k)));
int main() {
  std::atomic<S> a;
  std::atomic<int *> p;
  std::printf("%d %d\n", a.is_lock_free(), p.is_lock_free());
}

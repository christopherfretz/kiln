// [atomics.types.generic] std::atomic<T> of 1-, 2-, 4- and 8-byte
// structs under libstdc++: `alignas(sizeof(T)) T _M_i` reached through
// std::__addressof gives a naturally aligned, lock-free inline atomic
// (clang EmitPointerWithAlignment, WP-819), so the program links without
// libatomic (run tests never link -latomic); also an over-aligned member.
// (compare_exchange is left out: libstdc++'s __atomic_impl helpers take a
// plain `_Tp*`, so clang calls libatomic there too.)
//@ expect: run
//@ args: -stdlib=libstdc++
//@ strict-diags
//@ stdout: 1 2 4 8 | 3 6 12 24 | 1 1 1 1 | 7
#include <atomic>
#include <cstdio>
struct T1 { char a; };
struct T2 { char a, b; };
struct T4 { short a, b; };
struct T8 { int a, b; };
struct O { char c; alignas(16) std::atomic<T8> m; };
template <class T> int roundtrip(std::atomic<T> &a, T v) {
  T e = a.load();
  a.store(v);
  T old = a.exchange(e);
  a.store(old);
  return a.load().a;
}
int main() {
  std::atomic<T1> a1{T1{1}};
  std::atomic<T2> a2{T2{2, 0}};
  std::atomic<T4> a4{T4{4, 0}};
  std::atomic<T8> a8{T8{8, 0}};
  std::printf("%d %d %d %d | ", a1.load().a, a2.load().a, a4.load().a, a8.load().a);
  std::printf("%d %d %d %d | ", roundtrip(a1, T1{3}), roundtrip(a2, T2{6, 0}),
              roundtrip(a4, T4{12, 0}), roundtrip(a8, T8{24, 0}));
  std::printf("%d %d %d %d | ", a1.is_lock_free(), a2.is_lock_free(),
              a4.is_lock_free(), a8.is_lock_free());
  O o{'x', T8{7, 0}};
  std::printf("%d\n", o.m.load().a);
}

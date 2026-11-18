// [atomics.types.operations] (P0528): compare_exchange compares value
// representations, so padding bits do not take part, and the strong form
// never fails spuriously. libstdc++ clears the padding of the expected
// value with __builtin_clear_padding; a std::nullptr_t member's bytes are
// padding (GCC), and when kiln kept them the exchange failed on equal
// values whose nullptr_t bytes differed. A padded scalar struct alongside.
// Found running GCC's C++ testsuite. clang 22 has no
// __builtin_clear_padding, so libstdc++ cannot ignore the padding there
// and the exchange fails (clang is wrong too). WP-1694.
//@ expect: run
//@ std: c++20
//@ skip(clang): clang has no __builtin_clear_padding, so libstdc++'s compare_exchange compares padding
//@ oracle: gcc
//@ strict-diags
//@ stdout: n1 p1 3
#include <atomic>
#include <cstddef>
#include <cstdio>
#include <cstring>

struct N { std::nullptr_t n; };
struct alignas(8) P { char c; int i; };

int main() {
  std::atomic<N> a(N{nullptr});
  N e;
  std::memset(&e, 0xff, sizeof e);
  std::printf("n%d ", a.compare_exchange_strong(e, N{nullptr}));
  std::atomic<P> b(P{1, 2});
  P f;
  std::memset(&f, 0xff, sizeof f);
  f.c = 1;
  f.i = 2;
  std::printf("p%d ", b.compare_exchange_strong(f, P{3, 4}));
  std::printf("%d\n", b.load().c);
}

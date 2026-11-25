// [temp.deduct.general]/2-3, /8, [temp.over]/1: substituting the explicit
// `T = int` into the pattern of the trailing function parameter pack
// (`L<Ts, typename T::type>...`) fails, so the candidate is removed
// from the overload set although `Ts` deduces empty; the call takes the
// other overload, a worse match (pointer conversion to `const void*`).
// With `T = A`, whose `type` exists, the pack candidate stays and wins.
//@ expect: run
//@ strict-diags
//@ oracle: gcc
//@ stdout: 2 1
#include <cstdio>
template <class...> struct L {};
struct A { using type = int; };
template <class T, class... Ts> int p(T*, L<Ts, typename T::type>...) { return 1; }
template <class T> int p(const void*) { return 2; }
int main() {
  int* ip = nullptr;
  A* ap = nullptr;
  int r = p<int>(ip);
  int s = p<A>(ap);
  std::printf("%d %d\n", r, s);
}

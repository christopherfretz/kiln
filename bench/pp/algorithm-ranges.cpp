//@ mode: pp
//@ args: -stdlib=libc++
// Preprocess-only cost of <algorithm> + <ranges>.
#include <algorithm>
#include <ranges>

int main() {
  int a[] = {3, 1, 2};
  std::ranges::sort(a);
  return a[0] - 1;
}

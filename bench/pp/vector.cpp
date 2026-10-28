//@ mode: pp
//@ args: -stdlib=libc++
// Preprocess-only cost of the <vector> include graph.
#include <vector>

int main() {
  std::vector<int> v{1, 2, 3};
  return static_cast<int>(v.size()) - 3;
}

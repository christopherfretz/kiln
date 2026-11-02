// [range.elementsof], [coro.generator.promise]/9-12: libstdc++'s
// std::generator<T> (GCC 14 <generator>, C++23) yields
// std::ranges::elements_of(r), both of a nested generator (recursively,
// resumed through symmetric transfer) and of a plain range (element by
// element). Nesting assigns the promise's std::variant frame stack,
// which needs deduction against a base spelled through an alias
// template ([temp.deduct.call]/4.3, WP-381).
// Matches clang 19.
//@ expect: run
//@ std: c++23
//@ stdout: 0 1 0 2 0 1 0 | 7 8 1 2 9 | 2036
#include <generator>
#include <ranges>
#include <vector>
#include <cstdio>

std::generator<int> count(int lo, int hi) {
  for (int i = lo; i < hi; ++i)
    co_yield i;
}

// A recursive generator: elements_of(generator) yields the nested
// generator's elements through symmetric transfer, without copying.
std::generator<int> tree(int depth) {
  if (depth == 0) {
    co_yield 0;
    co_return;
  }
  co_yield std::ranges::elements_of(tree(depth - 1));
  co_yield depth;
  co_yield std::ranges::elements_of(tree(depth - 1));
}

// elements_of a non-generator range (yielded element by element).
std::generator<const int&> mixed() {
  std::vector<int> v{7, 8};
  co_yield std::ranges::elements_of(v);
  co_yield std::ranges::elements_of(count(1, 3));
  co_yield 9;
}

int main() {
  for (int x : tree(2))
    std::printf("%d ", x);
  std::printf("|");
  for (int x : mixed())
    std::printf(" %d", x);
  int sum = 0;
  for (int x : tree(10))
    sum += x;
  std::printf(" | %d\n", sum);
}

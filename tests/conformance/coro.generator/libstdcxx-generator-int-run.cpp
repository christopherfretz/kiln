// [coro.generator.class], [coro.generator.promise], [coro.generator.iterator]:
// std::generator<int> from libstdc++ (GCC 14 <generator>, C++23): a
// coroutine yields through `co_yield`, is iterated by range-for and through
// range adaptors, yields references to locals, prvalue tuples and rvalue
// references, and is abandoned part-way (its frame destroyed at a
// suspension point). Matches clang 19.
//@ expect: run
//@ std: c++23
//@ stdout: 45 18 a b 10 7 1 | 0 1 2 destroyed
#include <generator>
#include <ranges>
#include <tuple>
#include <cstdio>

std::generator<int> count(int n) {
  for (int i = 0; i < n; ++i)
    co_yield i;
}

struct tag {
  char c;
};

std::generator<const tag&> tags() {
  tag a{'a'};
  co_yield a;
  co_yield tag{'b'};
}

std::generator<std::tuple<int, long>> tup() {
  co_yield {1, 2L};
  co_yield std::tuple<int, long>(3, 4);
}

std::generator<int&&> rref() {
  int x = 3;
  co_yield std::move(x);
  co_yield 4;
}

struct noisy {
  ~noisy() { std::printf(" destroyed"); }
};

std::generator<int> endless() {
  noisy n;
  for (int i = 0;; ++i)
    co_yield i;
}

int main() {
  long sum = 0;
  for (int x : count(10))
    sum += x;
  std::printf("%ld", sum);
  long ad = 0;
  for (int x : count(10) | std::views::transform([](int i) { return i * 2; }) | std::views::take(3))
    ad += x;
  std::printf(" %ld", ad + 12);
  for (const tag& x : tags())
    std::printf(" %c", x.c);
  long t = 0;
  for (auto [a, b] : tup())
    t += a + b;
  std::printf(" %ld", t);
  long r = 0;
  for (int&& x : rref())
    r += x;
  std::printf(" %ld", r);
  auto g = count(3);
  auto it = g.begin();
  std::printf(" %d |", static_cast<int>(*it == 0));
  for (int x : endless()) {
    if (x == 3)
      break;
    std::printf(" %d", x);
  }
  std::printf("\n");
}

// [std.modules], [ranges]: `import std;` under C++26 provides std::ranges
// algorithms and views; non-exported libc++ internals stay invisible.
//@ expect: run
//@ std: c++26
//@ args: -stdlib=libc++ -Wno-reserved-module-identifier
//@ module: std=%{libcxx_modules}/std.cppm
//@ stdout: 4 16 36
import std;

int main() {
  std::vector<int> v{5, 2, 6, 1, 4, 3};
  std::ranges::sort(v);
  auto sq = v | std::views::filter([](int x) { return x % 2 == 0; })
              | std::views::transform([](int x) { return x * x; });
  bool first = true;
  for (int x : sq) {
    std::cout << (first ? "" : " ") << x;
    first = false;
  }
  std::cout << '\n';
  return std::ranges::is_sorted(v) ? 0 : 1;
}

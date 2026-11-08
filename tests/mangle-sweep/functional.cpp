#include <functional>
#include <string>
#include <map>
#include <unordered_map>
struct S { int f(int x) const { return x; } int m = 2; };
int fn_sweep() {
  std::function<int(int)> f = [](int x) { return x + 1; };
  auto b = std::bind(std::plus<int>(), std::placeholders::_1, 2);
  auto bf = std::bind_front(std::minus<>(), 5);
  S s;
  auto mf = std::mem_fn(&S::f);
  auto i = std::invoke(&S::f, s, 3) + std::invoke(&S::m, s) + std::invoke(f, 2);
  auto r = std::ref(s);
  auto cr = std::cref(i);
  auto nf = std::not_fn([](int x) { return x > 0; });
  std::hash<std::string> h;
  std::unordered_map<std::string, int> um{{"a", 1}};
  std::map<int, int, std::greater<>> m{{1, 2}};
  std::reference_wrapper<int> rw = i;
  return f(1) + b(1) + bf(1) + mf(s, 1) + i + r.get().m + cr + nf(1) + (int)h("x") + um["a"] + m[1] + rw
         + std::invoke_r<int>(f, 3) + std::equal_to<>{}(1, 1) + std::identity{}(3) + std::ranges::less{}(1, 2);
}

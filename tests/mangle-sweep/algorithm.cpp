#include <algorithm>
#include <numeric>
#include <vector>
#include <string>
#include <functional>
int algo_sweep(std::vector<int>& v, std::vector<std::string>& s) {
  std::sort(v.begin(), v.end());
  std::stable_sort(s.begin(), s.end());
  std::sort(v.begin(), v.end(), std::greater<>());
  std::ranges::sort(v);
  std::ranges::sort(s, {}, &std::string::size);
  auto it = std::find(v.begin(), v.end(), 3);
  auto it2 = std::find_if(v.begin(), v.end(), [](int x) { return x > 2; });
  auto it3 = std::ranges::find(v, 4);
  auto lb = std::lower_bound(v.begin(), v.end(), 2);
  auto ub = std::ranges::upper_bound(v, 2);
  bool bs = std::binary_search(v.begin(), v.end(), 3);
  auto mm = std::minmax_element(v.begin(), v.end());
  auto mx = std::ranges::max(v);
  auto mn = std::min({1, 2, 3});
  std::vector<int> o(v.size());
  std::copy(v.begin(), v.end(), o.begin());
  std::ranges::copy(v, o.begin());
  std::transform(v.begin(), v.end(), o.begin(), [](int x) { return x + 1; });
  std::ranges::transform(v, o.begin(), std::negate<>());
  std::reverse(o.begin(), o.end());
  std::rotate(o.begin(), o.begin() + 1, o.end());
  std::unique(o.begin(), o.end());
  std::fill(o.begin(), o.end(), 0);
  std::ranges::fill(o, 1);
  std::iota(o.begin(), o.end(), 0);
  auto acc = std::accumulate(v.begin(), v.end(), 0);
  auto red = std::reduce(v.begin(), v.end());
  std::nth_element(o.begin(), o.begin() + 1, o.end());
  std::partial_sort(o.begin(), o.begin() + 1, o.end());
  std::make_heap(o.begin(), o.end());
  std::push_heap(o.begin(), o.end());
  std::pop_heap(o.begin(), o.end());
  std::sort_heap(o.begin(), o.end());
  bool eq = std::equal(v.begin(), v.end(), o.begin());
  bool rq = std::ranges::equal(v, o);
  auto cnt = std::count_if(v.begin(), v.end(), [](int x) { return x; });
  auto any = std::ranges::any_of(v, [](int x) { return x > 3; });
  std::ranges::reverse(o);
  auto [a, b] = std::ranges::minmax(v);
  auto rr = std::ranges::remove(o, 1);
  o.erase(rr.begin(), rr.end());
  std::erase_if(o, [](int x) { return x == 2; });
  auto pp = std::partition(o.begin(), o.end(), [](int x) { return x & 1; });
  auto inn = std::inner_product(v.begin(), v.end(), v.begin(), 0);
  std::vector<int> ps(v.size());
  std::partial_sum(v.begin(), v.end(), ps.begin());
  std::ranges::for_each(v, [&](int x) { a += x; });
  auto c = std::clamp(5, 1, 3);
  auto lx = std::lexicographical_compare(v.begin(), v.end(), o.begin(), o.end());
  std::ranges::shuffle;
  return *it + *it2 + *it3 + *lb + *ub + bs + *mm.first + mx + mn + acc + red + eq + rq + (int)cnt + any + a + b
         + (pp == o.end()) + inn + c + lx + ps[0];
}

// [allocator.members], [expr.const]/5.20: libc++'s std::vector and
// std::string allocate through std::allocator<T> and construct and destroy
// through std::construct_at / std::destroy_at, so they work in constant
// evaluation (the reduction of libc++'s span.elem/at.pass.cpp is the span
// over a vector). spec/oven.md "Transient allocation".
//@ expect: compile-ok
//@ args: -stdlib=libc++
#include <memory>
#include <span>
#include <string>
#include <utility>
#include <vector>

constexpr int vector_sum(int n) {
  std::vector<int> v;
  for (int i = 0; i < n; ++i)
    v.push_back(i);
  int s = 0;
  for (int x : v)
    s += x;
  return s;
}
static_assert(vector_sum(1000) == 499500);

constexpr bool vector_of_strings() {
  std::vector<std::string> v{"alpha", "beta"};
  v.emplace_back(40, 'x');
  v.erase(v.begin());
  return v.size() == 2 && v[0] == "beta" && v[1].size() == 40;
}
static_assert(vector_of_strings());

constexpr std::size_t string_append() {
  std::string s(100, 'x');
  s += " and a tail that is long enough to grow the buffer";
  return s.size();
}
static_assert(string_append() == 150);

constexpr bool span_at() {
  std::vector vec{0, 1, 2, 3, 4, 5, 9084};
  std::span vec_span{vec};
  return vec_span.at(0) == 0 && vec_span.at(6) == 9084 && std::as_const(vec_span).at(1) == 1;
}
static_assert(span_at());


// [vector.modifiers] insert(pos, n, value), [vector.capacity] reserve,
// [unique.ptr] (constexpr since C++23),
// [optional.assign], [class.union.general]/2: constant evaluation of a
// fill-insert into a std::vector with spare capacity, whose
// implementation copies the value into a temporary held in a union and
// copy-assigns elements from it; each element type owns an allocation
// (a nested vector, a string beyond the small-string size, a
// unique_ptr-like owner through resize/assign), and an optional holding
// a vector is assigned (another union-held value). The reads of the
// union member go through `const T *const &`, a type similar to the
// member's (spec/oven.md "Unions", WP-1490). Reduced from real code.
//@ expect: compile-ok
//@ strict-diags
//@ std: c++23
#include <memory>
#include <optional>
#include <string>
#include <vector>

struct Row { std::string name; std::vector<std::string> list; };
constexpr std::size_t row_list() {
  std::vector<Row> v;
  v.reserve(16);
  for (int i = 0; i < 6; ++i) v.push_back({"a", {"x"}});
  v.insert(v.begin() + 1, 2, Row{"c", {"y", "z"}});
  return v.size() + v[1].list.size() + v[2].name.size();
}
static_assert(row_list() == 8 + 2 + 1);

struct Ints { std::vector<int> v; };
constexpr int ints() {
  std::vector<Ints> v;
  v.reserve(8);
  for (int i = 0; i < 3; ++i) v.push_back({{i}});
  v.insert(v.begin() + 1, 3, Ints{{7, 8, 9}});
  return int(v.size()) + v[1].v[2] + v[3].v[0] + v[4].v[0];
}
static_assert(ints() == 6 + 9 + 7 + 1);

constexpr std::size_t long_strings() {
  std::vector<std::string> v;
  v.reserve(8);
  for (int i = 0; i < 3; ++i) v.push_back(std::string(40, 'a'));
  v.insert(v.begin(), 2, std::string(50, 'b'));
  return v.size() + v[0].size() + v[2].size() + v[4].size();
}
static_assert(long_strings() == 5 + 50 + 40 + 40);

struct Owner {
  std::unique_ptr<int> p;
  constexpr Owner() = default;
  constexpr explicit Owner(int x) : p(std::make_unique<int>(x)) {}
  constexpr Owner(const Owner &o) : p(o.p ? std::make_unique<int>(*o.p) : nullptr) {}
  constexpr Owner &operator=(const Owner &o) {
    p = o.p ? std::make_unique<int>(*o.p) : nullptr;
    return *this;
  }
};
constexpr int owners() {
  std::vector<Owner> v;
  v.reserve(8);
  v.resize(2, Owner(3));
  v.insert(v.begin() + 1, 2, Owner(4));
  int a = int(v.size()) + *v[0].p + *v[1].p + *v[3].p;
  v.assign(3, Owner(5));
  return a + int(v.size()) + *v[2].p;
}
static_assert(owners() == 4 + 3 + 4 + 3 + 3 + 5);

constexpr std::size_t optionals() {
  std::optional<std::vector<int>> a, b(std::vector<int>{1, 2, 3});
  a = b;
  std::vector<int> &r = *a;
  r.push_back(4);
  b = std::vector<int>{5};
  a = b;
  return a->size() + (*b)[0];
}
static_assert(optionals() == 1 + 5);

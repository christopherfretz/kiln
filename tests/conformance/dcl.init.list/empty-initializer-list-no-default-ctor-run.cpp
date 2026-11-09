// [dcl.init.list]/3.6 (CWG1070): an empty list for a std::initializer_list<E>
// specialization without a default constructor is not value-initialization
// (3.5 needs a default constructor) but list-initialization from an empty
// backing array: size() is 0, as a member, as an initializer-list
// constructor's argument, and in a constant expression.
//@ std: c++14
//@ expect: run
//@ stdout: 0 0 0 0 1
#include <cstdio>
namespace std {
typedef decltype(sizeof 0) size_t;
template <typename T> class initializer_list {
  const T *p;
  size_t n;
  constexpr initializer_list(const T *p, size_t n) : p(p), n(n) {}

public:
  constexpr size_t size() const { return n; }
  constexpr const T *begin() const { return p; }
  constexpr const T *end() const { return p + n; }
};
} // namespace std
struct A {
  std::size_t n;
  A(std::initializer_list<int> l) : n(l.size()) {}
};
struct C {
  std::initializer_list<int> a;
  A b;
};
constexpr std::size_t count(std::initializer_list<long> l) {
  std::size_t k = 0;
  for (long v : l)
    k += v != 0;
  return k + l.size();
}
static_assert(count({}) == 0, "");
int main() {
  C c = {};
  A d{};
  std::initializer_list<int> e = {};
  std::printf("%zu %zu %zu %zu %d\n", c.a.size(), c.b.n, d.n, e.size(),
              e.begin() == e.end());
}

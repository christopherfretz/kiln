//@ expect: ast
//@ ast-filter: use
namespace std {
template <class E> class initializer_list {
  const E* b;
  decltype(sizeof 0) n;

public:
  constexpr initializer_list() : b(nullptr), n(0) {}
};
} // namespace std
struct V {
  V(std::initializer_list<int> l, int k = 0);
  V(int, int);
};
struct W {
  W(const std::initializer_list<double>& l);
};
void take(std::initializer_list<long>);
void use() {
  std::initializer_list<int> a = {1, 2, 3};
  const std::initializer_list<int>& r = {4};
  auto d = {5u, 6u};
  V v{1, 2};
  V c(3, 4);
  V p({5});
  W w = {1.0, 2.0};
  take({7, 8});
}

// [support.initlist]/1: an implementation's std::initializer_list may hold
// a pointer to the backing array's first element and either its size or a
// pointer past its end; list-initialization fills either shape, also in a
// constant expression ([dcl.init.list]/5).
namespace std {
template<class E> class initializer_list {
  const E *b;
  const E *e;
public:
  constexpr initializer_list() : b(nullptr), e(nullptr) {}
  constexpr decltype(sizeof 0) size() const { return e - b; }
  constexpr const E *begin() const { return b; }
  constexpr const E *end() const { return e; }
};
}

constexpr int sum(std::initializer_list<int> l) {
  int s = 0;
  for (int x : l)
    s += x;
  return s;
}
static_assert(sum({1, 2, 3}) == 6);
static_assert(sum({}) == 0);
constexpr int count() {
  std::initializer_list<int> l = {4, 5};
  return int(l.size());
}
static_assert(count() == 2);

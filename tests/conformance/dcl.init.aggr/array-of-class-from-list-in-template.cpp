// [dcl.init.aggr]/4, [dcl.array]/3, [temp.inst]: an array of unknown bound
// in a function template, list-initialized from braced elements whose
// constructors bind a temporary (`T(const int&)`), is list-initialized again
// in each specialization (the pattern's initializer is a full-expression
// with cleanups around the list; libc++ range.elements/iterator/decrement:
// `std::tuple<int> ts[] = {{1}, {2}, {3}};`).
//@ expect: run
template <class T> struct Box {
  T v;
  constexpr Box(const T& x) : v(x) {}
};
template <class... Ts> struct Pack {
  int n;
  constexpr Pack(const Ts&... xs) : n((0 + ... + xs)) {}
};

template <class I> constexpr int sum() {
  Box<int> bs[] = {{1}, {2}, {3}};
  Box<I> cs[] = {{4}, {5}};
  Pack<int, I> ps[] = {{1, 2}, {3, 4}};
  static_assert(sizeof(bs) == 3 * sizeof(Box<int>));
  static_assert(sizeof(cs) == 2 * sizeof(Box<I>));
  int r = 0;
  for (auto& b : bs) r += b.v;
  for (auto& c : cs) r += c.v;
  for (auto& p : ps) r += p.n;
  return r;
}

static_assert(sum<int>() == 25);

int main() { return sum<long>() == 25 ? 0 : 1; }

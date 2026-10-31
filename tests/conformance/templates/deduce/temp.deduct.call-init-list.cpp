// [temp.deduct.call]/1: a braced list deduces through std::initializer_list<P>.
//@ xfail(kiln): building std::initializer_list objects from a braced list is not implemented (pre-existing gap, not deduction)
namespace std {
template <class E> class initializer_list {
  const E* b; decltype(sizeof 0) n;
public:
  constexpr initializer_list() : b(nullptr), n(0) {}
  constexpr decltype(sizeof 0) size() const { return n; }
  constexpr const E* begin() const { return b; }
  constexpr const E* end() const { return b + n; }
};
}
template <class T> constexpr T sum(std::initializer_list<T> l) {
  T s = 0;
  for (const T* p = l.begin(); p != l.end(); ++p) s += *p;
  return s;
}
static_assert(sum({1, 2, 3}) == 6);
static_assert(sum({1.5, 2.5}) == 4.0);

// [over.match.oper]/3 with [temp.deduct.call]: operator function templates, non-member and member.
template <class T> struct V { T x; };
template <class T> constexpr V<T> operator+(V<T> a, V<T> b) { return {a.x + b.x}; }
static_assert((V<int>{1} + V<int>{2}).x == 3);
struct S {
  int k;
  template <class T> constexpr int operator()(T t) const { return k + t; }
  template <class T> constexpr int operator[](T t) const { return k * t; }
};
static_assert(S{2}(3) == 5);
static_assert(S{2}[3] == 6);

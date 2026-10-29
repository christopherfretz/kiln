// [temp.explicit]/1, /10: an explicit instantiation definition of a class
// template specialization instantiates its members; an explicit
// instantiation declaration ('extern template') suppresses implicit
// instantiation of their definitions.
template <class T> struct Box {
  T value;
  constexpr T get() const { return value; }
  static constexpr unsigned long size() { return sizeof(T); }
};
template struct Box<int>;
extern template struct Box<long>;
static_assert(Box<int>::size() == 4);
static_assert(sizeof(Box<long>) == 8);
constexpr Box<int> b = {7};
static_assert(b.value == 7);

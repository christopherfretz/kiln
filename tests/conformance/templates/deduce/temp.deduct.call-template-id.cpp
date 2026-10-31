// [temp.deduct.type]/8: TT<T> against a class template specialization.
template <class T> struct box { T v; };
template <class T> constexpr T unbox(box<T> b) { return b.v; }
static_assert(unbox(box<int>{4}) == 4);
static_assert(unbox(box<char>{'x'}) == 'x');

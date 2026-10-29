// [temp.deduce.type]/8 (TT<T>), [temp.arg.template]: a template template
// parameter and its argument deduced in a partial specialization.
template <class T> struct Box { T t; };
template <class T> struct Bag { T a, b; };
template <class T> struct unwrap { using type = T; static constexpr int depth = 0; };
template <template <class> class TT, class T> struct unwrap<TT<T>> {
  using type = typename unwrap<T>::type;
  static constexpr int depth = unwrap<T>::depth + 1;
};
static_assert(unwrap<int>::depth == 0);
static_assert(unwrap<Box<int>>::depth == 1);
static_assert(unwrap<Box<Bag<Box<char>>>>::depth == 3);
static_assert(sizeof(unwrap<Bag<Box<short>>>::type) == 2);

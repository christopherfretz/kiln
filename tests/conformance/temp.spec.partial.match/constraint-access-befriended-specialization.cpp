// [temp.spec.partial.match]/2, [class.friend]/2: the associated
// constraints of a partial specialization are checked as part of matching
// the specialization `traits<It>`; a class that befriends `traits<It>`
// (or the whole template) grants that check access to its private members.
// libc++ 22's `__product_iterator_traits` partial specialization is
// constrained on the private `zip_view::__iterator::__is_zip_view_iterator`
// (libcxx/iterators/product_iterator.pass.cpp).
//@ expect: run
template <class T> struct traits { static constexpr int size = 0; };
template <class I>
  requires I::is_it
struct traits<I> { static constexpr int size = 2; };

template <class U> struct Outer {
  class It {
    static constexpr bool is_it = true;
    friend struct traits<It>;
  };
};
static_assert(traits<Outer<int>::It>::size == 2);

template <class T> struct all;
class A {
  static constexpr bool is_it = true;
  template <class> friend struct all;
};
template <class I> struct all { static constexpr bool v = false; };
template <class I> requires(I::is_it) struct all<I> { static constexpr bool v = true; };
static_assert(all<A>::v);

// Not a friend: the constraint is a substitution failure, so the primary
// template is used.
class N {
  static constexpr bool is_it = true;
};
static_assert(traits<N>::size == 0);

// SFINAE detection through the specialization's completeness.
template <class T> struct only;
template <class T, unsigned long = 0> struct has { static constexpr bool value = false; };
template <class T> struct has<T, sizeof(only<T>) * 0> { static constexpr bool value = true; };
class B {
  static constexpr bool is_b = true;
  friend struct only<B>;
};
template <class T> requires T::is_b struct only<T> {};
static_assert(has<B>::value);
static_assert(!has<N>::value);

int main() {}

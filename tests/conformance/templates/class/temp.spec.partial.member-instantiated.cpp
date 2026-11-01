//@ expect: run
// [temp.spec.partial.member]/2, [temp.mem]: a partial specialization of
// a member class template is instantiated with its enclosing class
// template, so `In<T, B>` of `Outer<int>` matches `In<int, B>`; a
// partial specialization whose arguments are a pack (`Is<U>` against
// `class... Up`) is matched too (libc++ `tuple::_IsThisTuple`,
// `_EnableCtorFromUTypesTuple`).
template <class T> struct Outer {
  template <class A, class B> struct In { static const int v = 0; };
  template <class B> struct In<char, B> { static const int v = 1; };
  template <class B> struct In<T, B> { static const int v = 2; };
};
static_assert(Outer<int>::In<char, int>::v == 1);
static_assert(Outer<int>::In<int, int>::v == 2);
static_assert(Outer<int>::In<long, int>::v == 0);
static_assert(Outer<long>::In<long, char>::v == 2);

template <class... Tp> struct Tup {
  template <class... Up> struct Is { static const int v = 0; };
  template <class U> struct Is<U> { static const int v = 1; };
  template <class... Up> struct En { static const int v = Is<Up...>::v; };
  template <class O, class = void> struct Len { static const int v = 0; };
  template <class... Up>
  struct Len<Tup<Up...>, decltype(void(sizeof(char[sizeof...(Up) == sizeof...(Tp) ? 1 : -1])))> {
    static const int v = 1;
  };
};
static_assert(Tup<int>::Is<int>::v == 1);
static_assert(Tup<int>::Is<int, int>::v == 0);
static_assert(Tup<int>::En<int>::v == 1);
static_assert(Tup<int, long>::Len<Tup<char, char>>::v == 1);
static_assert(Tup<int, long>::Len<Tup<char>>::v == 0);

int main() { return Outer<int>::In<int, char>::v + Tup<int>::En<int, int>::v - 2; }

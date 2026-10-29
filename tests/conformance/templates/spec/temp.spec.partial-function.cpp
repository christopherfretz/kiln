// [temp.deduce.type]/8: function types and their parts deduced in partial
// specializations.
template <class T> struct ret { using type = void; };
template <class R, class A> struct ret<R(A)> { using type = R; };
template <class R, class A, class B> struct ret<R(A, B)> { using type = R; };
template <class T> struct arity { static constexpr int value = -1; };
template <class R> struct arity<R()> { static constexpr int value = 0; };
template <class R, class A> struct arity<R(A)> { static constexpr int value = 1; };
template <class R, class A> struct arity<R (*)(A)> { static constexpr int value = 11; };
static_assert(sizeof(ret<long(int)>::type) == 8);
static_assert(sizeof(ret<char(int, int)>::type) == 1);
static_assert(arity<int()>::value == 0);
static_assert(arity<int(char)>::value == 1);
static_assert(arity<int (*)(char)>::value == 11);
static_assert(arity<int>::value == -1);

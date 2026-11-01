// [temp.spec.partial.match], [class.member.lookup], [temp.arg]/2: in
// `X<A>::name` with a non-dependent `X<A>`, `name` is looked up in the
// specialization `X<A>` names: a member declared only by a partial or
// explicit specialization, or inherited from a base the instantiation
// names, is a type when it declares one. kiln's lookahead looks in the
// primary pattern, so such a qualifier is parsed ahead (clang annotates
// the scope token). libc++ `__atomic/memory_order.h:40`:
// `is_same<underlying_type<memory_order>::type, ...>`.
template <class A, class B> struct same { static constexpr bool value = false; };
template <class A> struct same<A, A> { static constexpr bool value = true; };

template <class A> struct cr;
template <class T> struct cr<T*> { using type = int; static constexpr int value = 4; };
template <> struct cr<char> { using type = long; };

static_assert(same<cr<int*>::type, int>::value);
static_assert(same<cr<char>::type, long>::value);
static_assert(sizeof(cr<int*>::type) == sizeof(int));
static_assert(sizeof cr<int*>::value == sizeof(int));
static_assert(cr<int*>::type(2) == 2);
static_assert((cr<int*>::type)3 == 3);
static_assert(static_cast<cr<char>::type>(5) == 5L);
static_assert(cr<int*>::value == 4);
static_assert((cr<int*>::value) + 1 == 5);
cr<int*>::type x = 1;

// Through a base chosen by a partial specialization.
template <class T, bool = __is_enum(T)> struct ui {};
template <class T> struct ui<T, true> { using type = __underlying_type(T); };
template <class T> struct ut : ui<T> {};
enum class E : short { a };
static_assert(same<ut<E>::type, short>::value);
static_assert(ut<E>::type(7) == 7);

int f() {
  cr<int*>::type y = 0;
  return y + sizeof(ut<E>::type);
}

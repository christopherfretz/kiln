// __builtin_common_type<Base, HasType, NoType, Ts...> (clang 20+;
// WP-1740), the builtin template libc++ 20+ derives std::common_type
// from: HasType<C> when Ts... have the common type C of
// [meta.trans.other]/3 (Note A: none for no type, common_type_t<T0, T0>
// for one, decay then `false ? declval<D1>() : declval<D2>()`, from
// C++20 COND-RES(CREF(D1), CREF(D2)), a left fold for more), a user
// specialization of the two-type case found through Base<T1, T2>, else
// NoType. Oracle-checked by hand with clang++-22 (22.1.8): same answers.
//@ requires: clang>=22
//@ std: c++20
template <class... T> struct common_type;
template <class... T> using common_type_t = typename common_type<T...>::type;
template <class T> struct type_identity { using type = T; };
struct empty {};
template <class... T>
struct common_type : __builtin_common_type<common_type_t, type_identity, empty, T...> {};

template <class T, class = void> constexpr bool has_type = false;
template <class T>
constexpr bool has_type<T, decltype(void(sizeof(typename T::type *)))> = true;

struct X {};
struct Y {};
template <> struct common_type<X, Y> { using type = int; };
template <> struct common_type<Y, X> { using type = int; };
struct B {};
struct Der : B {};
struct P {
  operator int() const;
};
struct W {
  W(const P &);
};

static_assert(__is_same(common_type_t<int>, int));
static_assert(__is_same(common_type_t<const int>, int));
static_assert(__is_same(common_type_t<int, long>, long));
static_assert(__is_same(common_type_t<int, double, char>, double));
static_assert(__is_same(common_type_t<int &, const int &>, int));
static_assert(__is_same(common_type_t<int[3], int *>, int *));
static_assert(__is_same(common_type_t<void, void>, void));
static_assert(__is_same(common_type_t<X, Y>, int));
static_assert(__is_same(common_type_t<const X &, Y>, int));
static_assert(__is_same(common_type_t<X, Y, long>, long));
static_assert(__is_same(common_type_t<Der *, B *>, B *));
static_assert(__is_same(common_type_t<Der, B>, B));
static_assert(__is_same(common_type_t<P, W>, W));
static_assert(!has_type<common_type<>>);
static_assert(!has_type<common_type<X, int>>);
static_assert(!has_type<common_type<int, void *>>);
static_assert(!has_type<common_type<X, Y, void *>>);
static_assert(__is_base_of(type_identity<long>, common_type<int, long>));
static_assert(__is_base_of(empty, common_type<X, int>));

template <class... T> constexpr bool common = has_type<common_type<T...>>;
static_assert(common<int, long> && !common<int, X>);

// libc++ experimental/simd's generator-constructor pattern without the
// headers (WP-313; spec/sema-templates.md §11.4): a constrained
// constructor template whose constraint folds a variable template over an
// index pack, each element a partial specialization selected through
// `void_t<decltype(lib::declval<G>()(lib::integral_constant<size_t, I>()))>`.
// Copying a `simd` considers that constructor with `G = simd&`, so every
// copy instantiates N failing partial-specialization matches; constructing
// from a lambda instantiates N matching ones and their `can_broadcast`
// initializers. Distinct closure types make every (T, G, N) new. As in
// libc++, the library is a namespace whose names the patterns qualify
// (`lib::...`, so substitution makes elaborated types sharing one
// qualifier), and the translation unit holds a header's worth of overload
// sets. Measures variable template specialization lookup and partial-
// specialization matching, explicit-argument substitution of `declval`,
// fold expansion, type interning, and the per-frame instantiation meter
// (about 100k instantiations).

namespace lib {

using size_t = decltype(sizeof(0));

template <class T> T&& declval_(int);
template <class T> T declval_(long);
template <class T> decltype(lib::declval_<T>(0)) declval() noexcept;

template <class...> using void_t = void;
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { using type = T; };
template <bool B, class T = void> using enable_if_t = typename lib::enable_if<B, T>::type;

template <class T, T V> struct integral_constant {
  static constexpr T value = V;
  constexpr operator T() const noexcept { return V; }
};
template <class T, T... I> struct integer_sequence {};
template <size_t... I> using index_sequence = lib::integer_sequence<size_t, I...>;
template <size_t N>
using make_index_sequence = __make_integer_seq<lib::integer_sequence, size_t, N>;

template <class T, class U> inline constexpr bool is_same_v = __is_same(T, U);
template <class F, class T> inline constexpr bool is_convertible_v = __is_convertible(F, T);
template <class T> inline constexpr bool is_unsigned_v = __is_unsigned(T);
template <class T>
inline constexpr bool is_vectorizable_v = __is_arithmetic(T) && !lib::is_same_v<T, bool>;

template <class From, class To, class = void>
inline constexpr bool non_narrowing_v = false;
template <class From, class To>
inline constexpr bool
    non_narrowing_v<From, To, lib::void_t<decltype(To{lib::declval<From>()})>> = true;

template <class T, class U>
inline constexpr bool can_broadcast_v =
    (lib::is_vectorizable_v<U> && lib::non_narrowing_v<U, T>) ||
    (!lib::is_vectorizable_v<U> && lib::is_convertible_v<U, T>) || lib::is_same_v<U, int> ||
    (lib::is_same_v<U, unsigned int> && lib::is_unsigned_v<T>);

template <class T, class G, size_t I, class = void>
inline constexpr bool well_formed_v = false;
template <class T, class G, size_t I>
inline constexpr bool well_formed_v<
    T, G, I,
    lib::void_t<decltype(lib::declval<G>()(lib::integral_constant<size_t, I>()))>> =
    lib::can_broadcast_v<T, decltype(lib::declval<G>()(lib::integral_constant<size_t, I>()))>;

template <class T, class G, size_t... I>
constexpr bool can_generate(lib::index_sequence<I...>) {
  return (true && ... && lib::well_formed_v<T, G, I>);
}
template <class T, class G, size_t N>
inline constexpr bool can_generate_v = lib::can_generate<T, G>(lib::make_index_sequence<N>());

template <class T, size_t N> struct simd {
  T v[N];
  constexpr simd() : v{} {}
  template <class G, lib::enable_if_t<lib::can_generate_v<T, G, N>, int> = 0>
  constexpr explicit simd(G&&) noexcept : v{} {}
};

// A header's worth of overload sets (libc++'s math and string overloads):
// 8,192 names, two declarations each.
#define KILN_OV(n) int n(int); long n(long);
#define KILN_X4(m, p) m(p##0) m(p##1) m(p##2) m(p##3)
#define KILN_X16(m, p) KILN_X4(m, p##0) KILN_X4(m, p##1) KILN_X4(m, p##2) KILN_X4(m, p##3)
#define KILN_X64(m, p) KILN_X16(m, p##0) KILN_X16(m, p##1) KILN_X16(m, p##2) KILN_X16(m, p##3)
#define KILN_X256(m, p) KILN_X64(m, p##0) KILN_X64(m, p##1) KILN_X64(m, p##2) KILN_X64(m, p##3)
#define KILN_X1024(m, p) \
  KILN_X256(m, p##0) KILN_X256(m, p##1) KILN_X256(m, p##2) KILN_X256(m, p##3)
KILN_X1024(KILN_OV, fa_)
KILN_X1024(KILN_OV, fb_)
KILN_X1024(KILN_OV, fc_)
KILN_X1024(KILN_OV, fd_)
KILN_X1024(KILN_OV, fe_)
KILN_X1024(KILN_OV, ff_)
KILN_X1024(KILN_OV, fg_)
KILN_X1024(KILN_OV, fh_)

} // namespace lib

// One test: a generator-constructed simd and two copies.
template <class T, lib::size_t N, int K> constexpr int run() {
  auto gen = [](auto i) { return T(i + K); };
  lib::simd<T, N> a(gen);
  lib::simd<T, N> b(a);
  lib::simd<T, N> c = b;
  static_assert(lib::can_generate_v<T, decltype(gen), N>);
  static_assert(!lib::can_generate_v<T, lib::simd<T, N>&, N>);
  return int(N);
}

template <class T, int K> constexpr int widths() {
  return run<T, 4, K>() + run<T, 8, K>() + run<T, 16, K>() + run<T, 32, K>();
}

template <int K> constexpr int types() {
  return widths<signed char, K>() + widths<short, K>() + widths<int, K>() +
         widths<long, K>() + widths<long long, K>() + widths<unsigned char, K>() +
         widths<unsigned short, K>() + widths<unsigned, K>() + widths<unsigned long, K>() +
         widths<float, K>() + widths<double, K>() + widths<char, K>();
}

static_assert(types<0>() + types<1>() + types<2>() + types<3>() + types<4>() + types<5>() ==
              6 * 12 * 60);

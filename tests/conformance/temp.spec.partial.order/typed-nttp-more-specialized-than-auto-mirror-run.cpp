// [temp.spec.partial.order]/1, [temp.deduct.partial], [temp.deduct.type]/20:
// the mirror cases of typed-nttp-more-specialized-than-auto-run.cpp. A
// non-type parameter of a fixed type does not deduce from a synthesized
// argument whose type is a placeholder's unique type, while a placeholder
// (`auto`, `decltype(auto)`, `auto...`) or a parameter typed by another one
// (`T E`, whose `T` deduces from the argument's type) deduces from a
// synthesized `int`: the typed specialization is the more specialized one,
// whichever is declared first. Only arguments that both specializations
// match are used (for `os<'c'>` clang and GCC disagree on whether `int E`
// matches; WP-1236 follow-up).
//@ std: c++20
//@ expect: run
//@ stdout: 2 2 2 1 2 2 2 2
template <auto...> struct os { static constexpr int v = 0; };
template <int E> struct os<E> { static constexpr int v = 2; };
template <auto E> struct os<E> { static constexpr int v = 1; };

template <typename, auto...> struct ts { static constexpr int v = 0; };
template <typename S, int E> struct ts<S, E> { static constexpr int v = 2; };
template <typename S, auto E> struct ts<S, E> { static constexpr int v = 1; };

// A parameter typed by another one.
template <class, auto...> struct rs { static constexpr int v = 0; };
template <class T, auto E> struct rs<T, E> { static constexpr int v = 1; };
template <class T, T E> struct rs<T, E> { static constexpr int v = 2; };

// `decltype(auto)`.
template <auto...> struct qs { static constexpr int v = 0; };
template <decltype(auto) E> struct qs<E> { static constexpr int v = 1; };
template <int E> struct qs<E> { static constexpr int v = 2; };

// A pack of `auto` against a pack of `int`.
template <class, auto...> struct ps { static constexpr int v = 0; };
template <class S, auto E, auto... Es> struct ps<S, E, Es...> { static constexpr int v = 1; };
template <class S, int E, int... Es> struct ps<S, E, Es...> { static constexpr int v = 2; };

extern "C" int printf(const char *, ...);
int main() {
  printf("%d %d %d %d %d %d %d %d\n", os<5>::v, ts<int, 5>::v, rs<int, 5>::v, rs<long, 5>::v,
         qs<5>::v, ps<int, 1>::v, ps<int, 1, 2>::v, ps<int, 1, 2, 3>::v);
}

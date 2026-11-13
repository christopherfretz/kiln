// [temp.spec.partial.order]/1, [temp.deduct.partial], [temp.deduct.type]/20:
// `template <int E> os<E>` is more specialized than `template <auto E>
// os<E>`: `auto E` deduces from the synthesized `int` value, while `int E`
// does not deduce from a synthesized `auto` value (its unique type is not
// `int`). A `char` or `unsigned` value matches only `auto E`: the type of
// `int E` differs from the type the primary's `auto` takes
// ([temp.deduct.type]/20; GCC 16.1 agrees, clang 19/22 pick `int E`;
// WP-1405). Found while reducing WP-1217 (mp-units' quantity
// specifications).
//@ std: c++20
//@ oracle: gcc
//@ skip(clang): clang 19/22 match a partial specialization whose non-type parameter type differs from the primary's
//@ expect: run
//@ stdout: 2 1 2 1
template <auto...> struct os { static constexpr int v = 0; };
template <auto E> struct os<E> { static constexpr int v = 1; };
template <int E> struct os<E> { static constexpr int v = 2; };

template <typename, auto...> struct ts { static constexpr int v = 0; };
template <typename S, auto E> struct ts<S, E> { static constexpr int v = 1; };
template <typename S, int E> struct ts<S, E> { static constexpr int v = 2; };

extern "C" int printf(const char *, ...);
int main() { printf("%d %d %d %d\n", os<5>::v, os<'c'>::v, ts<int, 5>::v, ts<int, 5u>::v); }

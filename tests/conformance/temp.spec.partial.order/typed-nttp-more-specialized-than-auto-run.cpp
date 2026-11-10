// [temp.spec.partial.order]/1, [temp.deduct.partial], [temp.deduct.type]/20:
// `template <int E> os<E>` is more specialized than `template <auto E>
// os<E>`: `auto E` deduces from the synthesized `int` value, while `int E`
// does not deduce from a synthesized `auto` value (its unique type is not
// `int`). Found while reducing WP-1217 (mp-units' quantity
// specifications); kiln calls the two ambiguous.
//@ std: c++20
//@ expect: run
//@ xfail(kiln): WP-1217 follow-up, partial ordering of class partial specializations does not check a deduced non-type argument's type against its parameter's (deduced_nttp_type_mismatch is only used against the primary)
//@ stdout: 2 2 2 2
template <auto...> struct os { static constexpr int v = 0; };
template <auto E> struct os<E> { static constexpr int v = 1; };
template <int E> struct os<E> { static constexpr int v = 2; };

template <typename, auto...> struct ts { static constexpr int v = 0; };
template <typename S, auto E> struct ts<S, E> { static constexpr int v = 1; };
template <typename S, int E> struct ts<S, E> { static constexpr int v = 2; };

extern "C" int printf(const char *, ...);
int main() { printf("%d %d %d %d\n", os<5>::v, os<'c'>::v, ts<int, 5>::v, ts<int, 5u>::v); }

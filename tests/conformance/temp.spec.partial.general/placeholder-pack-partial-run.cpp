// [temp.spec.partial.general]/9.2, [temp.deduct.type]/20: a partial
// specialization of a template whose non-type pack is `auto...` may give
// the pack a placeholder type that is more specific (`auto *...`); the
// primary's `auto` element deduces from it, so the partial is more
// specialized and is chosen for pointer arguments (WP-1105).
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 1 2 0
extern "C" int printf(const char *, ...);
template <auto... Vs> struct S { static constexpr int v = 0; };
template <auto *...Ps> struct S<Ps...> { static constexpr int v = 1; };
template <auto V, auto... Vs> struct T { static constexpr int v = 0; };
template <auto *P, auto *...Ps> struct T<P, Ps...> { static constexpr int v = 2; };
int a, b;
int main() {
  printf("%d %d %d %d %d\n", S<1, 2>::v, S<&a, &b>::v, S<>::v, T<&a, &b>::v,
         T<&a, 1>::v);
}

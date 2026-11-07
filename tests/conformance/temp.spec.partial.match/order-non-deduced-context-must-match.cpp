// [temp.spec.partial.order]/1, [temp.deduct.partial]/12, [temp.deduct.type]/1:
// partial ordering deduces `B`'s parameters from `A`'s arguments, and the
// deduced arguments substituted into `B` must give `A`'s arguments again,
// non-deduced contexts included. `X<M, when<c>>` is not at least as
// specialized as `X<T, when<f<T>::value>>` (the value `f<T>::value` is a
// non-deduced context that does not match `c`), so the second partial
// specialization is the more specialized one and `zero<int>` is not
// ambiguous (Boost.Hana's `when<condition>` dispatch).
//@ expect: run
//@ stdout: 0 1 2
extern "C" int printf(const char*, ...);

template <class T, class U> struct same { static constexpr bool value = false; };
template <class T> struct same<T, T> { static constexpr bool value = true; };
template <class T> struct arith { static constexpr bool value = true; };
template <> struct arith<void> { static constexpr bool value = false; };
template <bool condition> struct when;

template <class M, class = void> struct zero : zero<M, when<true>> {};
template <class M, bool c> struct zero<M, when<c>> {
  static int apply() { return 1; }
};
template <class T>
struct zero<T, when<arith<T>::value && !same<T, bool>::value>> {
  static int apply() { return 0; }
};

// A pack of the primary's trailing parameter: `Impl<I, H>` against
// `Impl<I, H, T...>` compares element-wise.
template <unsigned long I, class... T> struct Impl;
template <unsigned long I, class H, class... T>
struct Impl<I, H, T...> : Impl<I + 1, T...> {};
template <unsigned long I, class H> struct Impl<I, H> {
  static constexpr unsigned long last = I;
};

int main() {
  printf("%d %d %lu\n", zero<int>::apply(), zero<void>::apply(),
         Impl<0, int, long, char>::last);
}

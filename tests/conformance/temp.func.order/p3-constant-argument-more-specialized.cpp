// [temp.func.order]/3, [temp.deduct.partial]/3: in partial ordering a
// non-type template parameter is replaced by a unique synthesized value,
// which no constant matches. So `f(ie<I, 1>)` deduces from `ie<I, N>`'s
// side and not the reverse: it is more specialized than `f(ie<I, N>)`
// and the call is not ambiguous. Also with a constant computed from a
// member of the enclosing class template (range-v3 concat_view's
// `advance_fwd_fun`).
//@ expect: run
//@ std: c++20
//@ strict-diags
using size_t = decltype(sizeof 0);
template <class I, size_t N> struct ie {};

template <typename I> constexpr int f(ie<I, 1>) { return 1; }
template <typename I, size_t N> constexpr int f(ie<I, N>) { return 2; }
static_assert(f(ie<int, 1>{}) == 1);
static_assert(f(ie<int, 0>{}) == 2);

template <class T> concept C = true;
template <class... Rs> struct S {
  static constexpr size_t last = sizeof...(Rs) - 1;
  struct F {
    template <typename I> requires C<I> constexpr int operator()(ie<I, last>) const { return 1; }
    template <typename I, size_t N> requires C<I> constexpr int operator()(ie<I, N>) const { return 2; }
  };
  static constexpr int g() { return F{}(ie<int, 1>{}) * 10 + F{}(ie<int, 0>{}); }
};
static_assert(S<int, int>::g() == 12);

int main() { return S<int, int>::g() - 12; }

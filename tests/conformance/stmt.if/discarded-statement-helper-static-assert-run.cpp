// [stmt.if]/2, [temp.inst]/8: a downstream field-access macro
// (its exception header) in an ordinary member
// function picks, by `if constexpr` on the destination type, one of two
// accessor lambdas and passes it to the helper template `impl`, whose
// last branches static_assert for a lambda of the wrong shape. The
// discarded branch's `impl<D>(handle, ao)` is only referenced, not
// odr-used, so that specialization (whose static_assert would fail) is
// never instantiated; the taken branch's is, and reads 7. kiln
// instantiated both and failed the static_assert. (Reduced from the
// macro; std::same_as/remove_cvref_t/optional replaced by local ones.)
//@ expect: run
//@ exit: 7
template <class A, class B> inline constexpr bool same = false;
template <class A> inline constexpr bool same<A, A> = true;
template <class A, class B> concept same_as = same<A, B> && same<B, A>;
template <class T> struct rm { using type = T; };
template <class T> struct rm<T &> { using type = T; };
template <class T> struct rm<const T> { using type = T; };
template <class T> struct rm<const T &> { using type = T; };
template <class T> using remove_cvref_t = typename rm<T>::type;
template <class T> struct optional {
  T v;
  T &operator*() { return v; }
};

struct PV {};
struct CH {
  bool getQ(unsigned char *p) const noexcept {
    *p = 7;
    return true;
  }
};
struct H : CH {
  void setQ(unsigned char) {}
};
namespace I {
template <class T> inline constexpr bool always_false = false;
template <class T> concept IsPV = same_as<remove_cvref_t<T>, PV>;
template <class A, class F>
concept Opt = requires(A a, const CH &h) {
  { a(h) } -> same_as<optional<F>>;
};
template <class A, class F>
concept PtrInOut = requires(A a, const CH &h, F *d) {
  { a(h, d) } -> same_as<bool>;
};
template <class F, class A> F impl(const CH &h, A &&a) {
  if constexpr (Opt<A, F>) {
    return *a(h);
  } else if constexpr (PtrInOut<A, F>) {
    F v;
    a(h, &v);
    return v;
  } else if constexpr (IsPV<F>) {
    static_assert(always_false<F>, "pv");
  } else {
    static_assert(always_false<F>, "no");
  }
}
} // namespace I
#define GET(handle, N, dest)                                                   \
  do {                                                                         \
    using D = remove_cvref_t<decltype(dest)>;                                  \
    if constexpr (I::IsPV<D>) {                                                \
      auto ao = [](const auto &x) -> decltype(x.get##N()) {                    \
        return x.get##N();                                                     \
      };                                                                       \
      (dest) = I::impl<D>((handle), ao);                                       \
    } else {                                                                   \
      auto ap = [](const auto &x, D *p) -> decltype(x.get##N(p)) {             \
        return x.get##N(p);                                                    \
      };                                                                       \
      (dest) = I::impl<D>((handle), ap);                                       \
    }                                                                          \
  } while (false)
struct S {
  int run(const H &h);
};
int S::run(const H &h) {
  unsigned char q;
  GET(h, Q, q);
  return q;
}
int main() {
  H h;
  return S().run(h);
}

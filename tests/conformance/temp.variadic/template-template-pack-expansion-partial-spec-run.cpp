// [temp.variadic]/5, [temp.arg.template]/1: `ext<B, Bs...>`, a partial
// specialization whose arguments expand a pack of template template
// parameters, is checked by the expansion's pattern and matched
// element by element (a type-erasure library's interface list
// `__extends<_Base, _Bases...>`; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 0 1 3
extern "C" int printf(const char *, ...);

template <template <class> class... Bs> struct ext;
template <> struct ext<> { static constexpr int n = 0; };
template <template <class> class B, template <class> class... Bs>
struct ext<B, Bs...> {
  static constexpr int n = 1 + ext<Bs...>::n;
  using rest = ext<Bs...>;
};

template <class> struct a {};
template <class> struct b {};

int main() {
  printf("%d %d %d\n", ext<>::n, ext<a>::n, ext<a, b, a>::rest::n + 1);
}

// [expr.prim.id.unqual]/3, [temp.dep.expr]/3 (C++23, P0847R7): in a
// generic lambda whose explicit object parameter has a dependent type,
// a capture by copy is const in a specialization exactly when that
// specialization's object parameter refers to a const object: a
// `const Self&` makes it const, a `Self&` does not, whether the capture
// is explicit or implied by `=`. The use is type-dependent, so
// `decltype((x))` is recomputed per specialization.
//@ expect: run
//@ std: c++23
//@ strict-diags
extern "C" int printf(const char*, ...);
template <class T> struct is_c { static constexpr bool v = false; };
template <class T> struct is_c<const T&> { static constexpr bool v = true; };
template <class T> T&& decl();

template <bool B> struct bc { static constexpr bool v = B; };
void dependent() {
  int x = 0;
  auto a = [x](this const auto& self) { return bc<is_c<decltype((x))>::v>{}; };
  auto b = [x](this auto& self) { return bc<is_c<decltype((x))>::v>{}; };
  auto c = [=](this auto& self) { return bc<is_c<decltype((x))>::v>{}; };
  auto r = [&x](this auto& self) { return bc<is_c<decltype((x))>::v>{}; };
  static_assert(decltype(a())::v);
  static_assert(!decltype(b())::v);
  static_assert(decltype(decl<const decltype(b)&>()())::v);
  static_assert(!decltype(c())::v);
  static_assert(decltype(decl<const decltype(c)&>()())::v);
  static_assert(!decltype(decl<const decltype(r)&>()())::v);
}

constexpr int counter() {
  int x = 1;
  auto inc = [x](this auto& self) { return ++x; };
  inc();
  return inc();
}
static_assert(counter() == 3);

int main() {
  int x = 10;
  auto inc = [x](this auto& self) { return ++x; };
  auto get = [x](this const auto& self) { return x; };
  inc();
  int r = inc();
  printf("%d %d %d\n", r, get(), counter());
  return r == 12 && get() == 10 && counter() == 3 ? 0 : 1;
}

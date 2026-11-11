// [dcl.type.decltype], [temp.arg.nontype], [except.spec], [dcl.array],
// [temp.variadic], [module.interface], spec/modules.md "Expressions in
// types" (WP-1300): `decltype(nullptr)`, non-type template arguments
// written as expressions (through alias templates, dependent and not),
// `decltype(e)`, `noexcept(e)` (naming the function's parameters too),
// a dependent array bound and a pack expansion, exported from a module;
// under `--bmi-image` every one comes from the declaration image and the
// program behaves exactly as with the replayed source.
//@ expect: run
//@ std: c++20
//@ module: imge=Inputs/img-expr.cppm
//@ stdout: 2 5 3 1 2 4
import imge;
extern "C" int printf(const char *, ...);
struct Throws {
  Throws() noexcept(false) {}
  Throws &operator=(const Throws &) noexcept(false) { return *this; }
};
static_assert(sizeof(ie::nullptr_t) == sizeof(void *));
static_assert(ie::not_ptr<int>::value && !ie::not_ptr<int *>::value);
static_assert(ie::twice<3>::value == 6 && ie::three::value == 3);
static_assert(sizeof(ie::add(1, 2.0)) == sizeof(double) && sizeof(ie::minus1<short>) == sizeof(int));
static_assert(noexcept(ie::make<int>()) && !noexcept(ie::make<Throws>()));
static_assert(sizeof(ie::buf<short>::data) == 3 * sizeof(short));
static_assert(ie::dup<int, char>::n == 4);
int main() {
  ie::pair<int> a{1}, b{2};
  ie::pair<Throws> c, d;
  static_assert(noexcept(swap(a, b)) && !noexcept(swap(c, d)));
  swap(a, b);
  printf("%d %d %d %d %d %d\n", ie::g(), ie::add(2, 3), int(ie::three()), b.a, ie::make<int>(),
         ie::dup<char, int>::n);
  return 0;
}

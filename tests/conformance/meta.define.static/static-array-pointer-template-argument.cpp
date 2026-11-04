// [meta.define.static] (P3491R3 §3.5.1), [temp.arg.nontype], [temp.inst]:
// a pointer into a define_static_array object is a valid template
// argument, and substituting that template parameter into an expression
// gives a pointer constant that a constant expression can read through:
// `T[I]...` with `const P* T` the array names its elements, which convert
// to class-type template arguments. A binding generator passes its
// lifted keep-alive pairs this way (a `{const P* data; unsigned char n;}`
// set from define_static_array, expanded as `check<Fn, Ties[I]...>`).
//@ args: -freflection -I tests/support
//@ skip(clang): clang 19 does not implement P2996 reflection
//@ expect: compile-ok
//@ strict-diags
#include "kiln_meta.h"

struct P {
  int nurse, patient;
};
struct Set {
  const P* data;
  unsigned char n;
};
constexpr P pairs[] = {{0, 1}, {2, 3}, {4, 5}};
constexpr Set lift() {
  auto s = std::define_static_array(pairs);
  return {s.data(), static_cast<unsigned char>(s.size())};
}
constexpr Set ties = lift();

template <P... Ks> consteval int sum() { return (0 + ... + (Ks.nurse * 10 + Ks.patient)); }
template <const P* T, int... I> consteval int at() { return sum<T[I]...>(); }
static_assert(at<ties.data, 0, 1, 2>() == 1 + 23 + 45);
template <const P* T> consteval int second() { return T[1].patient + T->nurse; }
static_assert(second<ties.data>() == 3);
template <const P* T> struct Holder {
  static constexpr int first = T[0].patient;
};
static_assert(Holder<ties.data>::first == 1);

int main() {}

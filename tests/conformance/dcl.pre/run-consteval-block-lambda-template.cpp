// [dcl.pre]/P3289R1, [temp.inst]/3, [expr.prim.lambda]: a consteval block
// in a lambda body substituted at instantiation is evaluated once per
// specialization; one whose evaluation succeeds (its failing branch not
// taken for these arguments) leaves the closure fully usable, at
// translation time and at run time: called, copied, returned from a
// function template and stored in a variable template (WP-1257; the
// failing paths are consteval-block-lambda-no-cascade.cpp and
// consteval-block-lambda-next-error.cpp).
//@ expect: run
//@ skip(clang): clang 19 has no consteval blocks (P3289R1)
//@ oracle: gcc
//@ strict-diags
//@ stdout: 4 1 8 2 3
#include <cstdio>

template <class T>
inline constexpr auto size_of = [](int scale) {
  consteval { if (sizeof(T) > 8) throw 1; }
  return static_cast<int>(sizeof(T)) * scale;
};

template <class T> constexpr auto make() {
  return [](int x) {
    consteval { if (sizeof(T) > 8) throw 2; }
    return x + 1;
  };
}

template <class T> int via_body(int x) {
  auto l = [x] {
    consteval { if (sizeof(T) > 8) throw 3; }
    return x * 2;
  };
  auto copy = l;
  return copy();
}

static_assert(size_of<int>(1) == 4);
static_assert(make<char>()(1) == 2);

int main(int argc, char **) {
  auto inc = make<int>();
  std::printf("%d %d %d %d %d\n", size_of<int>(argc), size_of<char>(argc),
              size_of<double>(argc), inc(argc), via_body<short>(argc) + 1);
  return 0;
}

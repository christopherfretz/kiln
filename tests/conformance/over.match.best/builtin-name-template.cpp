// [over.match.best]/2.4: a function template declared at file scope with a
// builtin's name does not hide the builtin. Both are candidates (clang
// declares the builtin in the translation unit at the template's
// redeclaration lookup); with `int*` arguments the template is the exact
// match, and inside it, with `void*` / `const void*` arguments, the tie goes
// to the non-template builtin, so the template runs once and does not recurse.
// libc++ `copy_move_trivial.pass.cpp` hijacks `std::copy`'s memmove this way.
//@ expect: run
#include <stddef.h>

static int called = 0;

template <class Dst, class Src>
constexpr void* __builtin_memmove(Dst* dst, Src* src, size_t count) {
  ++called;
  return __builtin_memmove(static_cast<void*>(dst), static_cast<const void*>(src), count);
}

int main() {
  int a[2] = {1, 2}, b[2] = {};
  __builtin_memmove(b, a, sizeof a);
  if (b[1] != 2 || called != 1)
    return 1;
  // A void* argument picks the builtin directly.
  __builtin_memmove(static_cast<void*>(b), static_cast<const void*>(a), sizeof a);
  return called == 1 ? 0 : 2;
}

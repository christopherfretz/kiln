// [dcl.type.elab]/3, [dcl.align], [cpp.predefined], [dcl.fct]/5: an
// elaborated class named in a member declaration, `aligned` attributes,
// predefined macros in templates, specializations for `__int128_t`, a
// `__builtin_va_list` typedef and parameters of array and function type
// from an included header (the shapes of <__locale_t.h>,
// <__stddef_max_align_t.h>, <limits>, <__type_traits/is_integral.h>,
// <cstdarg> and <cstdio>). With `--pch-diff`, the header's precompiled
// snapshot carries them (WP-1310).
//@ expect: run
#include "Inputs/predef_aligned.h"

int pa::vsum(int n, pa::va ap) {
  int s = 0;
  for (int i = 0; i < n; ++i)
    s += __builtin_va_arg(ap, int);
  return s;
}

static int sum(int n, ...) {
  pa::va ap;
  __builtin_va_start(ap, n);
  int s = pa::vsum(n, ap);
  __builtin_va_end(ap);
  return s;
}

int main() {
  pa::D d{7};
  pa::S s{{&d, nullptr}, 3};
  if (s.p[0]->v != 7 || s.k != 3)
    return 1;
  if (alignof(pa::max_align) != alignof(long double) &&
      alignof(pa::max_align) != alignof(long long))
    return 2;
  if (alignof(pa::W) < 8 || sizeof(pa::W) % 8 != 0)
    return 3;
  if (pa::L<int>::bits != 32 || pa::L<char>::bits != 8)
    return 4;
  if (pa::Big<int>::max != __INT_MAX__ / 4)
    return 5;
  if (!pa::is_wide<__int128>::value || !pa::is_wide<unsigned __int128>::value ||
      pa::is_wide<long>::value)
    return 6;
  if (sum(3, 1, 2, 3) != 6)
    return 7;
  int arr[3] = {4, 5, 6};
  if (pa::third(arr) != 6 || pa::second(0, arr, +[](int v) { return v * 2; }) != 10)
    return 8;
  return 0;
}

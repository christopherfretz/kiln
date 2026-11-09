// Coverage comparison program (scripts/coverage-compare.py): inline
// functions, a template and a class defined in a header, an unused inline
// function (a zero record) and an #if 0 block in the main file.
#include "header_functions.h"
#if 0
int dead() { return 1; }
#else
int live() { return 2; }
#endif
int main(int argc, char **) {
  Acc a;
  for (int i = -2; i < 3; ++i)
    a.add(clamp_add(i, argc, 2));
  return twice(a.total) + twice(0.5) == 7.0 + live() - 2 ? 0 : 1;
}

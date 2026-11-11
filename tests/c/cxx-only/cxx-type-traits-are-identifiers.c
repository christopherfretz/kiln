// C has no C++ type-trait keywords: clang keeps only __is_destructible,
// __is_nothrow_destructible and __is_bitwise_cloneable (and the C-only
// __builtin_types_compatible_p) as keywords in C, so the other trait and
// type-transform spellings are ordinary identifiers. A trait's value has
// type int in C (C17 6.5.8p6 gives the relational operators int, too).
//@ expect: run
//@ strict-diags
//@ stdout: 3 1 1
#include <stdio.h>
int main(void) {
  int __is_same = 1, __is_trivially_copyable = 1, __remove_cv = 1;
  printf("%d %d %d\n", __is_same + __is_trivially_copyable + __remove_cv,
         __is_destructible(int),
         sizeof(__is_destructible(int)) == sizeof(int));
  return 0;
}

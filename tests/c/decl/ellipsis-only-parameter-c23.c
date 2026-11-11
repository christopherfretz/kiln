// C23 6.7.7.4p1 (function declarators): a parameter-type-list may be `...`
// alone; va_start takes no second argument (C23 7.16.1.4).
//@ expect: run
//@ std: c23
//@ stdout: 15
#include <stdarg.h>
#include <stdio.h>
static int sum(...) {
  va_list ap;
  va_start(ap);
  int n = va_arg(ap, int), s = 0;
  for (int i = 0; i < n; i++) s += va_arg(ap, int);
  va_end(ap);
  return s;
}
int main(void) { printf("%d\n", sum(3, 4, 5, 6)); return 0; }

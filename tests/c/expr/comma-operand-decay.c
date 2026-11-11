// C17 6.5.17p2: the result of the comma operator has the type and value of
// its right operand, which undergoes the array-to-pointer, function-to-pointer
// and lvalue conversions (6.3.2.1p2-4) -- unlike C++, where the result is an
// lvalue of the right operand's type. WP-1285 (gcc.c-torture
// compile/20011106-1.c).
//@ std: c17
//@ expect: run
//@ strict-diags
//@ stdout: 7 1
#include <stdio.h>
void foo(void) {}
int arr[10];
const int ci = 4;
char check[sizeof(1, foo) == sizeof(void (*)(void)) ? 1 : -1];
_Static_assert(_Generic((0, arr), int *: 1, default: 0), "array decays");
_Static_assert(_Generic((0, foo), void (*)(void): 1, default: 0), "function decays");
_Static_assert(_Generic((0, ci), int: 1, default: 0), "qualifiers dropped");
int main(void) {
  int *p = ((void)0, arr);
  p[1] = 7;
  printf("%d %d\n", arr[1], (int)sizeof check);
  return 0;
}

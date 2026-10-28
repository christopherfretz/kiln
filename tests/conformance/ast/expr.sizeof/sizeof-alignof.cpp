//@ expect: ast
struct S { char c; double d; };
unsigned long sizes(int x) {
  int arr[7];
  return sizeof x + sizeof(S) + alignof(S) + sizeof arr + sizeof(int[3]) + alignof(double);
}
static_assert(sizeof(S) == 16);

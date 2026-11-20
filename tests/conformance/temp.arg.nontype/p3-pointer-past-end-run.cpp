// [temp.arg.nontype]/3, [basic.compound]/3: a pointer one past the end of
// an object, an array or a member array is a valid template argument for a
// pointer parameter; substituted into the template it is that pointer
// value (kiln reported "not yet implemented: non-type template argument of
// this type" for it, WP-1740).
//@ std: c++20
//@ expect: run
//@ stdout: 1 4 10 4 7 8
//@ stdout: 2 2 1
extern "C" int printf(const char *, ...);
struct S { int a[2]; } s = {{3, 4}};
struct T { int a; int b[2]; } t = {5, {6, 7}};
constexpr int arr[4] = {1, 2, 3, 4};
int x = 10;
template <const int *P> struct A { static int get() { return P[-1]; } };
template <int *P> int *g() { return P; }
template <const int *P> constexpr int back() { return P[-1]; }
static_assert(back<arr + 4>() == 4);
int main() {
  printf("%d %d %d %d %d %d\n", A<&arr[1]>::get(), A<arr + 4>::get(), A<&x + 1>::get(),
         A<s.a + 2>::get(), A<t.b + 2>::get(), A<&t.b[1]>::get() + 2);
  int *p = g<s.a + 2>();
  int *q = g<t.b + 2>();
  printf("%d %d %d\n", (int)(p - s.a), (int)(q - t.b), g<&x + 1>() == &x + 1);
}

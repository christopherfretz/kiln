// [temp.arg.nontype]/3, [basic.compound]/3: a pointer one past the end of an
// object, of an array or of a member array is a valid template argument.
// No subobject lies at its address inside the complete object's last
// member, so the `so` form ends in `p` (`soiL_Z1sE8pE`), as clang mangles
// it (WP-1740). A past-the-end pointer that is also the address of the
// next member (`&t.a + 1` is `&t.b[0]`) is not tested: kiln's pointer
// values do not keep which one was written.
struct S { int a[2]; } s = {{3, 4}};
struct T { int a; int b[2]; } t = {5, {6, 7}};
struct U { int a; char c; } u;
int arr[4];
int x;
template <int *P> int f() { return P[-1]; }
template <int *P> int *g() { return P; }
template <char *P> int h() { return 0; }
int use() {
  return f<arr + 4>() + f<&x + 1>() + f<s.a + 2>() + f<t.b + 2>() + f<&t.b[1]>() +
         *g<s.a + 2>() + *g<t.b + 2>() + h<&u.c + 1>();
}

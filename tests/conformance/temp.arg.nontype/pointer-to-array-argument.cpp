// [temp.arg.nontype]/2, [temp.param]/8: a template argument `&arr` of a
// pointer-to-array type (`int (*)[2]`) names the array itself; substituted
// into the template it is the address of the array, not the decayed pointer
// to its first element ([conv.array] applies only to a parameter of type
// `int *`).
//@ expect: run
//@ std: c++20
//@ stdout: 2 8 1 1 7 7
#include <cstdio>
#include <type_traits>
struct A { inline static int g[2] = {7, 8}; int l[4]; };
int h[3] = {1, 2, 3};
template<auto D> int kind() {
  if constexpr (std::is_member_object_pointer_v<decltype(D)>) return 1;
  else return 2;
}
template<auto D> int second() { return (*D)[1]; }
template<int (*P)[3]> int first() { return (*P)[0]; }
template<int *P> int decayed() { return *P; }
template<const int (*P)[2]> int qualified() { return (*P)[0]; }
int main() {
  std::printf("%d %d %d %d %d %d\n", kind<&A::g>(), second<&A::g>(), kind<&A::l>(),
              second<&h>() - first<&h>(), decayed<A::g>(), qualified<&A::g>());
}

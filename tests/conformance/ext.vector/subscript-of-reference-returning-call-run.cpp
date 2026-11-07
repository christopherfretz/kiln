// GNU vector extension: a subscript of a vector glvalue produced by a call
// returning a reference (`get(p)[1]`, `v[i][j]` on a `std::vector` of
// vectors) reads the lane from the vector the call refers to. kiln
// lowered the call's address as the vector and failed in IR lowering
// ("extractelement on a non-vector"). WP-726.
//@ std: c++20
//@ expect: run
//@ stdout: 2 7 3 4
#include <cstdio>
typedef int v4 __attribute__((vector_size(16)));
struct Box {
  v4 v;
  v4& get() { return v; }
};
v4& get(v4* p) { return *p; }
int main() {
  v4 x{1, 2, 3, 4};
  Box b{{5, 6, 7, 8}};
  v4 arr[2] = {{0, 0, 0, 0}, {0, 0, 0, 3}};
  int i = 3;
  std::printf("%d %d %d %d\n", get(&x)[1], b.get()[2], (true ? arr[1] : x)[i],
              get(&x)[i]);
}

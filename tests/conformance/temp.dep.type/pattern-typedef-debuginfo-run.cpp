// [temp.dep.type] at `-g` (WP-910): a member typedef of a class template
// named by a non-dependent type in an instantiated member (`size_type` in
// `size()`) keeps the template pattern's typedef. Its debug scope is the
// unit, not the dependent pattern class, as in clang `getContextDescriptor`.
// kiln used to lay out the pattern class for its debug info, which tripped
// an internal assertion on a member of dependent type (`std::array<T, 0>`).
//@ expect: run
//@ args: -g
//@ stdout: 3 0 4
extern "C" int printf(const char *, ...);
template <class T, int N> struct Traits { using Type = T[N]; };
template <class T> struct Traits<T, 0> { struct Type {}; };
template <class T, int N> struct Arr {
  typedef unsigned long size_type;
  typename Traits<T, N>::Type elems;
  size_type size() const { return N; }
  T *data() { return nullptr; }
};
Arr<int, 3> a3{{1, 2, 3}};
Arr<int, 0> a0;
int main() {
  Arr<char, 4> local{};
  printf("%lu %lu %lu\n", a3.size(), a0.size(), local.size());
  return a0.data() != nullptr;
}

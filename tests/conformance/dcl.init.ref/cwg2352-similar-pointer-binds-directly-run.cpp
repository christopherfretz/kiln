// [dcl.init.ref]/5.1 with CWG2352: a `const T *const &` bound to a `T *`
// lvalue refers to that pointer object (no temporary copy), so a later
// store through the pointer object is seen through the reference, and
// a `begin()` returning `const T *const &` to a member returns the
// member itself.
//@ expect: run
//@ stdout: 1 1 1 1 1
#include <cstdio>

template <class T> struct Vec {
  T *b = nullptr;
  const T *const &begin() const { return b; }
};
int x = 7, y = 9;
int *gp = &x;
const int *const &gr = gp;
int *arr[2] = {&x, &y};
const int *const (&ra)[2] = arr;

int main() {
  Vec<int> v;
  v.b = &x;
  const int *const &b = v.begin();
  v.b = &y;
  gp = &y;
  arr[1] = &x;
  std::printf("%d %d %d %d %d\n", &gr == &gp, *gr == 9, &b == &v.b, *b == 9, *ra[1] == 7);
}

// [lex.phases]/1.9: translation units are combined by linking. With
// `-flto=thin -fuse-ld=lld` (WP-873) each unit is compiled to an LLVM bitcode object
// after the pre-link pipeline, and the link (here with the driver's
// lld) runs LTO over both
// units, so the virtual call and the cross-unit calls below resolve.
//@ expect: run
//@ args: -flto=thin -fuse-ld=lld %{test_dir}/Inputs/lto-two-tu-other.cpp
//@ stdout: 49 8
#include <cstdio>
struct Shape { virtual ~Shape(); virtual int area() const = 0; };
Shape* make_square(int v);
int twice(int x);
int main() {
  Shape* s = make_square(7);
  std::printf("%d %d\n", s->area(), twice(4));
  delete s;
}

// Interface unit of module `img2` for image-name-clash.cpp: it declares
// `geo`, as `img` does, so an importer of both builds the second from
// its source (spec/modules.md §11.11 "Part 2a", ODR merging).
export module img2;
namespace geo {
export int mul(int a, int b) { return a * b; }
}
export int img2_only(int x);
int img2_only(int x) { return x - 1; }

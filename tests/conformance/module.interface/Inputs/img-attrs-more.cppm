// The module of image-attrs-more-run.cpp: attributed declarations from a
// header in the purview, the out-of-line ones defined here.
export module imgam;
export {
#include "img-attrs-more.h"
}
extern "C" int iam_real(int x) { return x * 3; }
extern "C" int iam_twice(int x) { return x * 2; }
extern "C" void *iam_resolve() { return (void *)iam_twice; }
extern "C" int iam_ov(int x) __attribute__((overloadable)) { return x + 100; }
extern "C" int iam_ov(long x) __attribute__((overloadable)) { return (int)x + 200; }
namespace iam {
int cleaned = 0;
} // namespace iam

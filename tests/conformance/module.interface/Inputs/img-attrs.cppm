// The module of image-attrs.cpp and image-attrs-run.cpp: attributed
// declarations from a header in the purview, defined here.
export module imga;
export {
#include "img-attrs.h"
}
namespace ia {
int tagged(int x) { return x * 10; }
Tagged make_tagged(int x) { return {x + 1}; }
int hidden_fn(int x) { return x - 1; }
int slow(int x) { return x * x; }
int relabeled(int x) { return x + 7; }
int second(int *p, int *q) { return *p + *q; }
} // namespace ia

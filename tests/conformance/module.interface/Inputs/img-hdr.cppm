// Interface unit of module `imgh` for image-header-run.cpp and
// image-header-errors.cpp (spec/modules.md §11.11 "File table", stage
// S7): its declarations come from a header #included in the purview, so
// their locations lie in another file of the file table.
export module imgh;
export {
#include "img-hdr.h"
}
export constexpr int sq(int x) { return x * x; }

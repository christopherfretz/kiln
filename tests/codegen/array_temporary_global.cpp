// [class.temporary]/6, [basic.start.term]/1: an array temporary of a
// class with a non-trivial destructor extended by a namespace-scope
// reference is destroyed at exit by a `__cxx_global_array_dtor` helper
// (clang `pushTemporaryCleanup` for `SD_Static`, `generateDestroyHelper`);
// a constructor that throws destroys the constructed elements.
struct T { T(int); ~T(); int v; };
using TA = T[2];
const T (&g)[2] = {7, 8};
const TA& g2 = TA{9, 10};
int r() { return g[0].v + g2[1].v; }

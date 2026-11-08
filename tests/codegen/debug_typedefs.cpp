// Debug info (WP-814): typedef and alias declarations as DIDerivedType typedefs.
//@ args: -g
typedef unsigned long size_type;
using Ptr = const char *;
struct S {
  using Inner = int;
  Inner v;
};
size_type len(Ptr p, S s) {
  size_type n = 0;
  while (p[n])
    ++n;
  return n + s.v;
}

// The header of pch-run.cpp: parameter attributes on templates and on a
// prior declaration, read back from a precompiled header.
template <int N> int load(int *p __attribute__((align_value(N)))) { return *p; }
template <class T> T first(T *p __attribute__((nonnull)), T *q __attribute__((annotate("pch", 1)))) {
  return *p + *q;
}
int prior(int *p __attribute__((nonnull)), int *q __attribute__((annotate("prior"))));

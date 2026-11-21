// The module of image-thread-local-inline-run.cpp: thread_local
// variables each importer defines itself (inline, templates, local
// statics of inline functions).
export module imgtli;
int trace(int v) { return v; }
export inline thread_local int inl = trace(3);
export inline __thread int st = 4;
export struct S {
  static inline thread_local int si = trace(5);
};
export template <class T> struct Box {
  static thread_local T v;
};
template <class T> thread_local T Box<T>::v = T(trace(6));
export template <class T> thread_local T vt = T(trace(7));
export template <class T> T bump() {
  static thread_local T n{};
  return ++n;
}
export inline int counter() {
  static thread_local int n = 0;
  return ++n;
}
export inline int *addr() { return &inl; }

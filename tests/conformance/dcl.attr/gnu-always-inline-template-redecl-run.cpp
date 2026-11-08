// A function template redeclared with `always_inline` is inlined at -O0
// even when its first declaration lacks the attribute (clang merges the
// attributes of every redeclaration; libstdc++ declares
// `make_format_args` before its `[[gnu::always_inline]]` definition;
// WP-818). The -O2 form for `noinline` is
// gnu-noinline-template-redecl-O2-run.cpp.
//@ expect: run
//@ std: c++20
template <class T> void* frame(T);
template <class T> [[gnu::always_inline]] inline void* frame(T) {
  return __builtin_frame_address(0);
}
void* plain(int);
[[gnu::always_inline]] inline void* plain(int) { return __builtin_frame_address(0); }
int main() {
  void* me = __builtin_frame_address(0);
  if (plain(0) != me) return 2;
  return frame(0) == me ? 0 : 1;
}

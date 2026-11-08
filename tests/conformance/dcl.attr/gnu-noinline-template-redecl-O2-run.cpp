// A function template redeclared `noinline` keeps it on its
// instantiation even when its first declaration lacks it (clang merges
// the attributes of every redeclaration onto the latest; WP-818), so at
// -O2 the call is not inlined into `main`.
//@ expect: run
//@ std: c++20
//@ args: -O2
template <class T> void* kept(T);
template <class T> [[gnu::noinline]] void* kept(T) { return __builtin_frame_address(0); }
int main() {
  void* me = __builtin_frame_address(0);
  return kept(0) != me ? 0 : 1;
}

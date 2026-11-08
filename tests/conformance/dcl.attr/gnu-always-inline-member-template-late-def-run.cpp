// A member function template of a class template takes its attributes
// from the member declaration made when the class specialization is
// instantiated (clang): an `always_inline` written only on an
// out-of-line definition seen after `S<char>` was instantiated does not
// apply to `S<char>`'s specializations, while `S<int>`, instantiated
// after the definition, gets it. libstdc++'s
// `basic_string<char>::__resize_and_overwrite` is the first case
// (WP-818).
//@ expect: run
//@ std: c++20
template <class T> struct S {
  template <class U> void* frame(U);
};
inline int early() { return sizeof(S<char>); }
template <class T> template <class U>
[[gnu::always_inline]] inline void* S<T>::frame(U) {
  return __builtin_frame_address(0);
}
int main() {
  void* me = __builtin_frame_address(0);
  if (S<int>{}.frame(0) != me) return 2;
  return S<char>{}.frame(0) != me ? 0 : 1;
}

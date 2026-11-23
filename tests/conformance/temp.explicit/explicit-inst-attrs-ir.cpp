// [temp.explicit], GNU attributes: the decl-specifier attributes of an
// explicit instantiation of a function, variable or member become the
// specialization's (clang `ActOnExplicitInstantiation`), so `weak` makes
// an explicit instantiation definition `weak` (and, being interposable,
// not inferred `nounwind`: clang `TryMarkNoThrow`), an `extern template`
// declaration `extern_weak`, and `visibility` hides it. An attribute
// after the declarator is dropped, as clang drops it. kiln used to keep
// none of them (reduced from real code; WP-2541).
//@ expect: ir
template <class T> void f(T) {}
template <class T> struct C {
  void m() {}
  static int x;
  static void sf() {}
};
template <class T> int C<T>::x = 1;
template <class T> T v = T();
extern template __attribute__((weak)) void f<int>(int);
template __attribute__((weak)) void f<long>(long);
template __attribute__((visibility("hidden"))) void f<short>(short);
template void f<char>(char) __attribute__((weak));
template struct __attribute__((visibility("hidden"))) C<long>;
template __attribute__((weak)) int v<int>;
template __attribute__((weak)) int C<int>::x;
template __attribute__((weak)) void C<int>::m();
template __attribute__((visibility("hidden"))) void C<char>::sf();
__attribute__((weak)) void g(int) {}
void use() { f(1); }

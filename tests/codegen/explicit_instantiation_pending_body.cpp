// [temp.explicit]/10: a constexpr member of an explicitly instantiated class
// is defined once, though a member emitted before it calls it. Its body calls
// G's complete-object constructor, an alias of the base one defined later in
// the TU, so it is finished only at the end of the module; until then it
// still counted as a declaration, and the call's deferred definition appended
// a second copy (with -g, a second DISubprogram, and LLVM dropped the TU's
// debug info). Reduced from `template class std::vector<T>;` (_M_fill_insert).
//@ args: -std=c++23 -g
struct G {
  G();
  ~G();
};
template <class T> struct V {
  constexpr void g(int n) { f(n); }
  constexpr void f(int n);
};
template <class T> constexpr void V<T>::f(int n) {
  if (n != 0) {
    G y;
  }
}
template struct V<int>;
G::G() {}
G::~G() {}

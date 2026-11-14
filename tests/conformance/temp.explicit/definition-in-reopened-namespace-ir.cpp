// [temp.explicit]/10, [temp.explicit]/12: an explicit instantiation
// definition that follows an explicit instantiation declaration, written
// in a reopened namespace (or right after a namespace that closed the
// enclosing scope), defines each member once, where it appears (clang
// `HandleTopLevelDecl`). kiln used to key "at the start of this
// namespace" and "after this namespace" the same, so a member function
// was built twice into one IR function, the second body after the first
// `ret`, each with its own `DISubprogram` (LLVM then dropped all of the
// module's debug info). Found building a large open-source code base;
// spec/landed/codegen/WP-1425.md.
//@ requires: clang>=22
//@ expect: ir
namespace n {
template <class T, bool B> struct Base {
  T *root;
  void addRoot(T *t) { root = t; }
  int get() const { return 1; }
  void reset() { addRoot(nullptr); }
  template <class U> int g(U) { return 3; }
  static int s;
  struct Nested { int h() { return 4; } };
};
template <class T, bool B> int Base<T, B>::s = 5;
template <class T> int ft(T) { return 6; }
template <class T> int vt = 7;
struct Block {};
// [temp.expl.spec]: an explicitly specialized member is an ordinary
// definition, not one the explicit instantiation provides.
template <> inline void Base<Block, false>::addRoot(Block *) {}
extern template class Base<Block, false>;
extern template int ft<long>(long);
struct Derived : Base<Block, false> {};
} // namespace n
namespace n { template class Base<Block, false>; }
namespace n { template int ft<long>(long); }
namespace n { template int Base<int, true>::g<int>(int); }
namespace n { template struct Base<short, true>::Nested; }
namespace n { template int vt<long>; }
extern "C++" { namespace n { template struct Base<char, true>; } }
namespace m { int z() { return 9; } }
// After a namespace, not at its start.
template int n::Base<short, true>::get() const;
namespace m { namespace k {} }
template struct n::Base<int, false>;
extern "C++" { namespace p { int y() { return 10; } } }
template int n::Base<long, false>::get() const;
// After a function explicitly instantiated before its template is
// defined (itself defined at the end of the translation unit).
template <class T> int late(T);
template int late<int>(int);
template int n::Base<char, false>::get() const;
template <class T> int late(T) { return 11; }

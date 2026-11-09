// [basic.stc.dynamic.deallocation]/5 valid destroying operator deletes
// (WP-1040): with a size and/or an alignment, variadic, with the
// injected-class-name or an alias of it in a class template, in a nested
// class of a class template, and a global placement form taking `void *`.
//@ std: c++26
//@ expect: compile-ok
//@ strict-diags
namespace std {
using size_t = decltype(sizeof(0));
enum class align_val_t : size_t;
struct destroying_delete_t { explicit destroying_delete_t() = default; };
inline constexpr destroying_delete_t destroying_delete{};
}
using std::size_t;
using std::align_val_t;
using dd_t = std::destroying_delete_t;
void operator delete(void *, dd_t);
struct A {
  void operator delete(A *, dd_t);
  void operator delete(A *, dd_t, size_t);
  void operator delete(A *, dd_t, align_val_t);
  void operator delete(A *, dd_t, size_t, align_val_t);
  void operator delete(A *, dd_t, size_t, ...);
  void operator delete(void *, size_t);
};
template<class T> using id_alias = T;
template<class T> struct B { void operator delete(B<T> *, dd_t); };
template<class T> struct C { void operator delete(id_alias<C> *, dd_t, size_t); };
template<class T> struct O { struct I { void operator delete(I *, dd_t); }; };
template<class T> struct P;
template<class T> struct P<T *> { void operator delete(P *, dd_t); };
void f(A *a, B<int> *b, C<int> *c, O<int>::I *i, P<int *> *p) {
  delete a;
  delete b;
  delete c;
  delete i;
  delete p;
}

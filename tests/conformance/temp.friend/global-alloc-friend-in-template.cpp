// [temp.friend]/1, [temp.inst]/4, [basic.stc.dynamic.general]/2,
// [basic.scope.scope]/4 (WP-2540): a friend `operator new` / `operator
// delete` in a class template declares the global function only when the
// class is instantiated. A namespace-scope declaration of it, before or
// after the instantiation, redeclares the implicit declaration and the
// instantiated friend, never the pattern's friend; new- and
// delete-expressions and `__builtin_operator_new` call the global functions.
// The plain, array, aligned and placement forms. The redeclaration chains
// are checked by `pattern_friend_is_not_the_global_alloc_function`
// (crates/kiln_parse/tests/new_delete.rs), the code by
// ir-global-alloc-friend-in-template.cpp.
//@ expect: compile-ok
//@ strict-diags
namespace std {
using size_t = decltype(sizeof 0);
enum class align_val_t : size_t {};
}
struct alignas(64) O { int x; };

template <class T> struct A {
  friend void *operator new(std::size_t);
  friend void operator delete(void *) noexcept;
  friend void *operator new[](std::size_t);
  friend void operator delete[](void *) noexcept;
  friend void *operator new(std::size_t, std::align_val_t);
  friend void operator delete(void *, std::align_val_t) noexcept;
  friend void *operator new(std::size_t, void *) noexcept;
  friend void operator delete(void *, void *) noexcept;
};

// The instantiation first, then the global declarations.
A<int> a;
void *operator new(std::size_t);
void operator delete(void *) noexcept;
void *operator new[](std::size_t);
void operator delete[](void *) noexcept;
void *operator new(std::size_t, std::align_val_t);
void operator delete(void *, std::align_val_t) noexcept;
void *operator new(std::size_t, void *) noexcept;
void operator delete(void *, void *) noexcept;

// The global declarations first, then the instantiation.
A<long> b;

void use(void *buf) {
  int *p = new int;
  delete p;
  int *q = new int[2];
  delete[] q;
  O *o = new O;
  delete o;
  int *r = new (buf) int;
  (void)r;
  void *s = __builtin_operator_new(4);
  __builtin_operator_delete(s);
}

// [expr.new]/20-28, [expr.delete]/9-11 (P2719, WP-1088): the IR of
// new- and delete-expressions that select type-aware allocation functions:
// the std::type_identity<T> argument is passed first (an empty class
// argument), then the size and alignment; the array form keeps its cookie,
// and a throwing initialization calls the matching delete.
//@ expect: ir
//@ min-oracle: 22
//@ requires: clang>=22
//@ args: -Wno-ext-cxx-type-aware-allocators
namespace std {
template <class T> struct type_identity {};
using size_t = decltype(sizeof 0);
enum class align_val_t : size_t {};
}

struct S {
  int x;
  S();
  ~S();
  void *operator new(std::type_identity<S>, std::size_t, std::align_val_t);
  void operator delete(std::type_identity<S>, void *, std::size_t, std::align_val_t);
  void *operator new[](std::type_identity<S>, std::size_t, std::align_val_t);
  void operator delete[](std::type_identity<S>, void *, std::size_t, std::align_val_t);
};

template <class T> void *operator new(std::type_identity<T>, std::size_t, std::align_val_t);
template <class T> void operator delete(std::type_identity<T>, void *, std::size_t, std::align_val_t);

S *one() { return new S; }
void drop(S *s) { delete s; }
S *many(int n) { return new S[n]; }
void drop_many(S *s) { delete[] s; }
int *global() { return new int(3); }
void drop_global(int *p) { delete p; }

// The declaration-extent table (spec/ast.md §19.2): the name range is the
// unqualified-id, after a qualifier whose start is `qual=`, for operator
// functions, destructors, conversion functions, literal operators and
// the template-id of a specialization.
//@ decl-extents: names.extents
//@ std: c++20
namespace n {
struct S {
  S operator+(const S &) const;
  int operator()(int) const;
  ~S();
  operator int *() const;
  static void *operator new[](unsigned long);
};
} // namespace n
n::S n::S::operator+(const n::S &) const { return *this; }
int n::S::operator()(int) const { return 0; }
n::S::~S() {}
n::S::operator int *() const { return nullptr; }
int operator""_k(unsigned long long);
template <class T> struct V {};
template <> struct V<int> {};
template <class T> struct V<T *> {};
template <class T> void h(T) {}
template <> void h<char>(char) {}
extern int gq;
int ::gq = 1;

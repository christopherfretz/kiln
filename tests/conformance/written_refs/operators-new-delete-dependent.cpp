// Dependent new- and delete-expressions (spec/ast.md §18.13.6, stage
// 6c): a new-expression whose type or operand is dependent, or a
// delete-expression whose operand is type-dependent ([temp.dep.expr]/3),
// selects its allocation or deallocation function at instantiation
// ([expr.new]/12, [expr.delete]/9). Its keyword is a DEPENDENT entry with
// no found set (the lookup is in the class's scope, then the global
// scope: nothing is looked up at the definition), and each
// specialization that reaches it a row naming the function selected,
// `how=qualified`: the class's own, or the global replaceable one (a
// function like any other, not `(builtin)`). `::new` and `::delete` keep
// their `::` lead. A template never instantiated and a discarded
// `if constexpr` branch ([stmt.if]/2) have the entry and no row; a class
// template's member keys its rows by the member's specialization.
//@ std: c++20
//@ written-refs: operators-new-delete-dependent.refs
using size_t = decltype(sizeof 0);
void *operator new(size_t, void *p) noexcept;

struct C {
  static void *operator new(size_t);
  static void *operator new[](size_t);
  static void operator delete(void *);
  static void operator delete[](void *);
};
struct Plain {
  int v;
};

template <class T> T *make() { return new T; }
template <class T> T *make_n(size_t n) { return new T[n]; }
template <class T> T *gmake() { return ::new T; }
template <class T> void kill(T *p) { delete p; }
template <class T> void kill_n(T *p) { delete[] p; }
template <class T> void gkill(T *p) { ::delete p; }
template <class T> T *place(void *b, T v) { return new (b) T(v); }
template <int N> int *bound() { return new int[N]; }
template <class T> T *never() { return new T; }
template <class T> void discard(T *p) {
  if constexpr (sizeof(T) > 1000)
    delete p;
}
template <class T> struct Box {
  T *p;
  Box() : p(new T) {}
  ~Box() { delete p; }
};

void use(void *buf) {
  kill(make<C>());
  kill(make<int>());
  kill_n(make_n<C>(2));
  kill_n(make_n<int>(2));
  gkill(gmake<C>());
  place(buf, 1);
  place(buf, Plain{});
  delete[] bound<3>();
  discard<int>(nullptr);
  Box<C> bc;
  Box<int> bi;
}

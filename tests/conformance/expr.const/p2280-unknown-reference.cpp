// [expr.const]/10 (C++23, P2280R4): a reference variable not usable in
// constant expressions (a parameter outside its call, an `extern`
// reference) is treated as binding to an unspecified object of the
// referenced type whose lifetime includes the whole evaluation. Naming it,
// taking its address, calling a member function that does not read the
// object, and pointer arithmetic and comparisons within it are constant;
// only an access of the object is not (clang 22's constexpr-unknown
// lvalue).
//@ expect: compile-ok
//@ strict-diags
//@ requires: clang>=22
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
template <class T> constexpr bool named(T &) { return true; }
template <class T> constexpr T *addr(T &t) { return &t; }

void array_param(int (&a)[4]) {
  static_assert(size(a) == 4);
  static_assert(sizeof(a) == 4 * sizeof(int));
  static_assert(&a[1] - &a[0] == 1);
  static_assert(a + 1 != a);
  static_assert(&a[3] > &a[1]);
  static_assert(addr(a) == &a);
}

struct S {
  int m, n;
  constexpr int three() const { return 3; }
  constexpr const S *self() const { return this; }
};
void class_param(const S &s) {
  static_assert(s.three() == 3);
  static_assert(&s == &s);
  static_assert(s.self() == &s);
  static_assert(&s.m == &s.m);
  static_assert(&s.n != &s.m);
}

void scalar_param(int &x, const int &c) {
  static_assert(named(x));
  static_assert(named(c));
  static_assert(addr(x) == &x);
}

// A local reference outside its frame, initialized from an unknown one.
void local_reference(int (&a)[2]) {
  int (&b)[2] = a;
  static_assert(size(b) == 2);
}

// A namespace-scope reference with no (or no constant) initializer.
extern int (&ea)[3];
static_assert(size(ea) == 3);
extern int &er;
static_assert(&er == &er);
static_assert(named(er));

// Dependent: checked at instantiation.
template <class T> void dependent(T &x) {
  static_assert(size(x) == 2);
  static_assert(&x[1] - &x[0] == 1);
}
template void dependent<int[2]>(int (&)[2]);
template void dependent<const char[2]>(const char (&)[2]);

template <size_t N> struct Fixed {};
void as_template_argument(int (&a)[5]) { Fixed<size(a)> f; (void)f; }

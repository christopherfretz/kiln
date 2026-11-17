// [expr.const]/10.1 (P2280R4, a defect report): `this` is a core constant
// expression when it is the object expression of a class member access,
// implicit or explicit, even outside a call of a constexpr member function;
// `*this` is then an object of unknown value, so calls of constexpr member
// functions that read nothing, of `this`'s members and bases, and
// passing an array member on are constant, here in C++11 and in a template
// argument. GCC 16.1 agrees; clang 22 rejects every one ("use of 'this'
// pointer is only allowed within the evaluation of a call to a 'constexpr'
// member function"). Found running GCC's C++ testsuite.
//@ std: c++11
//@ oracle: gcc
//@ xfail(clang): clang 22 does not implement the `this` rule of P2280R4
typedef decltype(sizeof 0) size_t;
template <class T, size_t N> constexpr size_t len(T (&)[N]) { return N; }
template <int> void tmpl() {}
template <int N> struct Array { constexpr int size() const { return N; } };
struct B { int k; constexpr int g() const { return 2; } };
struct S : B {
  int m;
  int arr[3];
  B b;
  Array<10> inner;
  constexpr int f() const { return 3; }
  constexpr const S *self() const { return this; }
  void implicit_call() { static_assert(f() == 3, ""); }
  void explicit_call() { static_assert(this->f() == 3, ""); }
  void member_call() { static_assert(b.g() == 2 && this->b.g() == 2, ""); }
  void base_call() { static_assert(g() == 2 && this->g() == 2, ""); }
  void array_member() { static_assert(len(arr) == 3 && len(this->arr) == 3, ""); }
  void unevaluated() { static_assert(sizeof(m) == sizeof(int), ""); }
  void identity() const { static_assert(this->self() == self(), ""); }
  void template_argument() {
    tmpl<inner.size()>();
    tmpl<this->inner.size()>();
  }
};
template <class T> struct Dep {
  Array<4> a;
  void g() { tmpl<a.size()>(); }
};
template struct Dep<int>;

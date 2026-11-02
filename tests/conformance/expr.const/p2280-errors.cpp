// [expr.const]/5.10, /10 (C++23, P2280R4): only a variable of reference type
// binds to an unknown object. A reference member of an unknown object is
// read from that object, so naming it reads the variable; `this` outside a
// constexpr member function's call is not a constant ([expr.const]/5.1); a
// pointer to the unknown object is not a constant result, and comparing it
// with null or another object, or reading through the reference, is not
// constant (clang 22's notes).
//@ expect: compile-fail
//@ strict-diags
//@ std: c++23
//@ requires: clang>=22
//@ diag: 46:39 error "static assertion expression is not an integral constant expression"
//@ diag: 49:17 note "in instantiation of member function 'Widget<3>::member' requested here"
//@ diag: 46:39 note "implicit use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 47:46 error "static assertion expression is not an integral constant expression"
//@ diag: 49:17 note "in instantiation of member function 'Widget<3>::explicit_this' requested here"
//@ diag: 47:46 note "use of 'this' pointer is only allowed within the evaluation of a call to a 'constexpr' member function"
//@ diag: 51:50 error "static assertion expression is not an integral constant expression"
//@ diag: 51:57 note "read of variable 'h' whose value is not known"
//@ diag: 51:31 note "declared here"
//@ diag: 52:55 error "static assertion expression is not an integral constant expression"
//@ diag: 52:57 note "read of variable 'h' whose value is not known"
//@ diag: 52:36 note "declared here"
//@ diag: 53:49 error "static assertion expression is not an integral constant expression"
//@ diag: 53:58 note "comparison between pointers to unrelated objects '&w' and 'nullptr' has unspecified value"
//@ diag: 54:53 error "static assertion expression is not an integral constant expression"
//@ diag: 54:62 note "comparison between pointers to unrelated objects '&a.elems[0]' and 'nullptr' has unspecified value"
//@ diag: 55:53 error "static assertion expression is not an integral constant expression"
//@ diag: 55:53 note "read of variable 'a' whose value is not known"
//@ diag: 55:34 note "declared here"
//@ diag: 56:51 error "constexpr variable 'p' must be initialized by a constant expression"
//@ diag: 56:51 note "pointer to subobject of 'a' is not a constant expression"
//@ diag: 56:27 note "declared here"
using size_t = decltype(sizeof 0);
template <class T, size_t N> constexpr size_t size(T (&)[N]) { return N; }
template <class T, size_t N> struct Array {
  T elems[N];
  constexpr size_t count() const { return N; }
  constexpr const T *data() const { return elems; }
  constexpr const T &operator[](size_t i) const { return elems[i]; }
};
struct Holder { int (&arr)[5]; Array<int, 3> &a; };
template <int N> struct Widget {
  constexpr int f() const { return N; }
  constexpr const Widget *self() const { return this; }
  void member() const { static_assert(f() == N); }
  void explicit_this() const { static_assert(this->f() == N); }
};
template struct Widget<3>;

void reference_member(Holder &h) { static_assert(size(h.arr) == 5); }
void reference_member_call(Holder &h) { static_assert(h.a.count() == 3); }
void null_compare(Widget<3> &w) { static_assert(w.self() != nullptr); }
void data_compare(Array<int, 4> &a) { static_assert(a.data() != nullptr); }
void element_read(Array<int, 4> &a) { static_assert(a[0] == 0); }
void pointer_result(int (&a)[4]) { constexpr int *p = a; }

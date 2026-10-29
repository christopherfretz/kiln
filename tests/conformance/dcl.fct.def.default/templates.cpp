// [dcl.fct.def.default]/1 in class templates: a constructor or operator= of
// a dependent class is checked when its class is instantiated (it may become
// a special member, clang SetDeclDefaulted); other functions and member
// templates are diagnosed in the pattern. X and X<P...> name the class in its
// own pattern ([temp.dep.type]/1), so these are copy/move members. An
// instantiation does not warn -Wdefaulted-function-deleted.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 29:14 error "only special member functions and comparison operators may be defaulted"
//@ diag: 30:37 error "only special member functions and comparison operators may be defaulted"
//@ diag: 43:3 error "only special member functions and comparison operators may be defaulted"
//@ diag: 49:15 note "in instantiation of template class 'Bad<int>' requested here"
template <class T, int N> struct A {
  A() = default;
  A(const A &) = default;
  A(A<T, N> &&) = default;
  A &operator=(const A<T, N> &) = default;
  A &operator=(A &&) & = default;
  ~A() = default;
  T t;
};
template <class T, int N, class... Ts> struct V {
  V(const V<T, N, Ts...> &) = default;
  V(V &&) = default;
  V &operator=(const V &) = default;
  V &operator=(V<T, N, Ts...> &&) = default;
};
template <class T> struct P {
  void f() = default;
  template <class U> P(const U &) = default;
  P(int) = default;
  P &operator=(int) = default;
  P(const P<int> &) = default;
  struct I {
    I(const P &) = default;
  };
};
template <class T> struct R {
  R() = default;
  T &r;
};
template <class T> struct Bad {
  Bad(int) = default;
};
A<int, 1> a;
A<long, 2> a2;
static_assert(sizeof(R<int>) == sizeof(int *));
R<int> *r;
static_assert(sizeof(Bad<int>) == 1);

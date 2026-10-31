// [temp.names]/3, /5, [temp.arg.explicit]: member template-ids whose
// template arguments are dependent, on dependent and non-dependent
// objects, through `operator->`, implicit `this`, and with `template`;
// only the templates of an overload set take explicit arguments.
//@ expect: compile-ok
struct X {
  template <class T> constexpr int f() const { return sizeof(T); }
  constexpr int f(int) const { return -1; }
  template <class T, class U> constexpr int f(U u) const { return sizeof(T) + u; }
  template <int N> constexpr int g() const { return N; }
  template <class T> static constexpr int s() { return 2 * sizeof(T); }
};
struct P {
  X x;
  constexpr const X* operator->() const { return &x; }
};
template <class T> constexpr int a(const X& x) { return x.f<T>(); }
template <int N> constexpr int b(const X& x) { return x.g<N + 1>(); }
template <class T> constexpr int c() { return X::s<T>(); }
template <class T> constexpr int d(const X* x) { return x->template f<T*>(); }
template <class U> constexpr int e(const U& u) { return u.template g<3>(); }
template <class U, class T> constexpr int h(const U& u) { return u.template f<T>(); }
template <class T> constexpr int i(P p) { return p->f<T>() + p->template f<T>(2); }
template <class T> constexpr int j(const X& x) { return x.f<T>(1); }
constexpr X gx{};
static_assert(a<int>(X{}) == 4);
static_assert(b<5>(X{}) == 6);
static_assert(c<short>() == 4);
static_assert(d<char>(&gx) == 8);
static_assert(e(X{}) == 3);
static_assert(h<X, long>(X{}) == 8);
static_assert(i<int>(P{}) == 10);
static_assert(j<char>(X{}) == 2);
template <class T> struct C {
  template <class U> constexpr int g() const { return sizeof(U) + sizeof(T); }
  constexpr int k() const { return g<T>() + this->g<T>() + this->template g<char>(); }
  template <class V> constexpr int m(const X& x) const { return x.f<V>() + x.f<T, V>(V{1}); }
};
static_assert(C<short>{}.k() == 11);
static_assert(C<short>{}.m<int>(X{}) == 7);
struct S {
  template <class U> constexpr int g() const { return sizeof(U); }
  template <class T> constexpr int k() const { return g<T>() + this->g<T*>(); }
};
static_assert(S{}.k<char>() == 9);

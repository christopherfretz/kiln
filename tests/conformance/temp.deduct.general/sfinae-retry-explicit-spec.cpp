// [temp.deduct.general]/8, [temp.spec.partial.match]/1 (WP-3697): a
// failed function template specialization caused by an ambiguous class
// template partial specialization is re-deduced after a later explicit
// specialization resolves the ambiguity, and the explicit specialization
// itself is valid and usable afterwards.
//@ expect: compile-ok
template <class T, class U> struct X {};
template <class T> struct X<T, int> {
  using t = int;
};
template <class U> struct X<int, U> {
  using t = int;
};
template <class T> auto f(T) -> typename X<T, int>::t;
long f(...);

long p = sizeof(f(1));
static_assert(sizeof(f(1)) == sizeof(long));

template <> struct X<int, int> {
  using t = int;
};

static_assert(sizeof(f(1)) == sizeof(int));
X<int, int> x_obj;
X<int, int>::t x_val = 1;

// A later partial specialization resolving a previously incomplete primary.
template <class T> struct Trait;
template <class T> auto h(T) -> typename Trait<T>::type;
long h(...);

static_assert(sizeof(h((int*)0)) == sizeof(long));
template <class T> struct Trait<T*> {
  using type = int;
};
static_assert(sizeof(h((int*)0)) == sizeof(int));

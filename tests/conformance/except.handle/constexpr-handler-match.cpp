// [except.handle]/3 in constant evaluation (C++26, P3068R6): the handler
// matching rules, including the cases clang trunk's implementation is more
// permissive about (a `T&` pointer handler does not convert, a function
// pointer does not convert to `void*`, a private or ambiguous base does not
// match). [except.throw]/3: the operand's array/function type decays.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags

struct B { int b; };
struct D : B {};
struct Priv : private B {};
struct L : B {};
struct R : B {};
struct Amb : L, R {};
struct M { int m; };
int fn();

template <class E, class H> constexpr int match(E e) {
  try {
    throw e;
  } catch (H) {
    return 1;
  } catch (...) {
    return 0;
  }
}

static_assert(match<int, const int>(1) == 1);
static_assert(match<int, const int &>(1) == 1);
static_assert(match<int, long>(1) == 0);
static_assert(match<D, B>(D{}) == 1);
static_assert(match<D, B &>(D{}) == 1);
static_assert(match<Priv, B &>(Priv{}) == 0);
static_assert(match<Amb, B &>(Amb{}) == 0);
static_assert(match<D *, B *>(nullptr) == 1);
static_assert(match<D *, B *const &>(nullptr) == 1);
static_assert(match<D *, B *&>(nullptr) == 0);
static_assert(match<Priv *, B *>(nullptr) == 0);
static_assert(match<int *, const int *>(nullptr) == 1);
static_assert(match<const int *, int *>(nullptr) == 0);
static_assert(match<int *, void *>(nullptr) == 1);
static_assert(match<int (*)(), void *>(&fn) == 0);
static_assert(match<int (*)(), int (*)()>(&fn) == 1);
static_assert(match<decltype(nullptr), const int *>(nullptr) == 1);
static_assert(match<decltype(nullptr), const int *const &>(nullptr) == 1);
static_assert(match<decltype(nullptr), const int *&>(nullptr) == 0);
static_assert(match<decltype(nullptr), int M::*>(nullptr) == 1);
static_assert(match<int M::*, int M::*>(&M::m) == 1);
static_assert(match<int, int *>(0) == 0);

// The operand decays: an array is thrown as a pointer to its first element.
constexpr int array_decays() {
  int a[3] = {4, 5, 6};
  try {
    throw a;
  } catch (int *p) {
    return p[1];
  }
  return 0;
}
static_assert(array_decays() == 5);

// The handler binds the converted pointer, not the thrown one.
constexpr bool base_pointer_value() {
  D d{};
  try {
    throw &d;
  } catch (B *p) {
    return p == static_cast<B *>(&d);
  }
  return false;
}
static_assert(base_pointer_value());

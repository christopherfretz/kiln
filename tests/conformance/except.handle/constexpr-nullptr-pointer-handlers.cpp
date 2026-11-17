// [except.handle]/3.4 in constant evaluation (C++26, P3068R6): a thrown
// `nullptr` is caught by a handler of type `cv T` or `const T&`, T a pointer
// or pointer-to-member type, as a null pointer of that type; not by an
// `int`, `bool` or class handler, nor by a `T&` pointer handler (3.4 needs
// `const T&`; a rethrown `nullptr` caught as `const char **&` is therefore
// not caught; GCC 16.1 catches both `T&` cases and also rejects the
// pointer-to-member handlers in constant evaluation).
// Found running GCC's C++ testsuite.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ std: c++26
struct S {
  int m;
  void f() {}
};
struct FromNull {
  constexpr FromNull(decltype(nullptr)) {}
};
template <class H> constexpr bool caught_as() {
  try {
    throw nullptr;
  } catch (H h) {
    return h == nullptr;
  } catch (...) {
    return false;
  }
}
template <class H> constexpr bool skipped() {
  try {
    try {
      throw nullptr;
    } catch (H) {
      return false;
    }
  } catch (decltype(nullptr) p) {
    return p == nullptr;
  }
}
static_assert(caught_as<int *>());
static_assert(caught_as<const char *>());
static_assert(caught_as<void (*)()>());
static_assert(caught_as<int S::*>());
static_assert(caught_as<void (S::*)()>());
static_assert(caught_as<int *const &>());
static_assert(caught_as<const char *const>());
static_assert(caught_as<void (*const &)()>());
static_assert(caught_as<int S::*const &>());
static_assert(caught_as<void (S::*const &)()>());
static_assert(skipped<int>());
static_assert(skipped<bool>());
static_assert(skipped<FromNull>());
static_assert(skipped<int *&>());
constexpr bool rethrown() {
  try {
    try {
      throw nullptr;
    } catch (const char **const &x) {
      if (x != nullptr)
        return false;
      throw;
    }
  } catch (const char **&) {
    return false;
  } catch (const char *const *w) {
    return w == nullptr;
  }
  return false;
}
static_assert(rethrown());

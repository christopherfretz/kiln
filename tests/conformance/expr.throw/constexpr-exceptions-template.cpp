// [expr.const]/10 (C++26, P3068R6), [except.handle]/3, [temp.inst]: throw
// and catch in instantiated function templates, with the handler's type
// dependent on a template parameter.
//@ skip(clang): neither clang 19 nor clang 22 implements P3068
//@ expect: compile-ok
//@ strict-diags

template <class T> struct Box { T v; };

template <class T, class U> constexpr int catches(T t) {
  try {
    throw t;
  } catch (const U &) {
    return 1;
  } catch (...) {
    return 0;
  }
}
static_assert(catches<int, int>(1) == 1);
static_assert(catches<int, long>(1) == 0);
static_assert(catches<Box<int>, Box<int>>(Box<int>{2}) == 1);
static_assert(catches<Box<int>, Box<long>>(Box<int>{2}) == 0);

struct B {};
struct D : B {};
static_assert(catches<D, B>(D{}) == 1);
static_assert(catches<D *, B *>(nullptr) == 1);

template <class T> constexpr T value_of(T t) {
  try {
    throw Box<T>{t};
  } catch (Box<T> b) {
    return b.v;
  }
  return T();
}
static_assert(value_of(3) == 3);
static_assert(value_of('x') == 'x');

// A pack expanded into throws, one try-block per element.
template <class... Ts> constexpr int count_caught(Ts... ts) {
  int n = 0;
  ((
       [&] {
         try {
           throw ts;
         } catch (Ts) {
           ++n;
         }
       }()),
   ...);
  return n;
}
static_assert(count_caught(1, 2.0, 'c', B{}) == 4);

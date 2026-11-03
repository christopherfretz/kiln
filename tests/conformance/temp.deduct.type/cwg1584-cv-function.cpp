// [temp.deduct.type]/8, CWG1584: `cv T` does not match a function type,
// so `const T*` does not deduce from `void (*)()` even though the
// pointee's qualifiers are otherwise ignored for a qualification
// conversion; nor does a nested `const T*` or a `cv T` partial
// specialization. `const T&` binding a function is unaffected (the
// reference's top-level qualifiers are dropped first).
//@ expect: compile-ok
//@ strict-diags
template <class T> constexpr int f(const T*) { return 1; }
constexpr int f(...) { return 0; }
template <class T> concept CallsF = requires(T t) { f<>(t); };
using F = void();
static_assert(f((int*)nullptr) == 1 && f((F*)nullptr) == 0);
static_assert(CallsF<int*> && !CallsF<F*>);

template <class T> constexpr int g(const T* const*) { return 1; }
constexpr int g(...) { return 0; }
static_assert(g((int* const*)nullptr) == 1 && g((F* const*)nullptr) == 0);

template <class T> struct X { static constexpr int v = 0; };
template <class T> struct X<const T> { static constexpr int v = 1; };
static_assert(X<const int>::v == 1 && X<F>::v == 0);

void fn();
template <class T> constexpr int h(const T&) { return 3; }
static_assert(h(fn) == 3);
template <class T> constexpr int p(T*) { return 2; }
static_assert(p(&fn) == 2);

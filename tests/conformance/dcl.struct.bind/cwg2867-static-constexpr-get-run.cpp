// CWG2867, [dcl.struct.bind]/4, [basic.start.static]/2: a structured binding
// with static or thread storage duration whose initializer and `get` calls
// are all constexpr-capable still keeps the initializer's temporaries alive
// until the last `get`: `e` refers to a temporary, so it is not constant-
// initialized and the declaration is initialized dynamically, with the
// temporaries destroyed after the `get` calls (an evaluation that ended the
// temporaries first would read -1). The constant-evaluated automatic form
// (`f`) agrees. Found running GCC's C++ testsuite.
//@ expect: run
//@ std: c++26
//@ strict-diags
//@ oracle: gcc
//@ xfail(clang): neither clang 19 nor clang 22 implements CWG2867 (the static_assert fails)
//@ stdout: 3 4 5 6 12
extern "C" int printf(const char*, ...);
namespace std { template<class T> struct tuple_size; template<unsigned long, class T> struct tuple_element;
template<class T> struct tuple_size<const T> : tuple_size<T> {};
template<unsigned long I, class T> struct tuple_element<I, const T> { using type = const typename tuple_element<I, T>::type; }; }
struct T { int v[2]; bool live = true; constexpr ~T() { live = false; } };
struct P {
  mutable const T* p;
  template <unsigned long I> constexpr int get() const {
    int r = p->live ? p->v[I] : -1;
    if (I == 1) p = nullptr;
    return r;
  }
};
template<> struct std::tuple_size<P> { static constexpr unsigned long value = 2; };
template<unsigned long I> struct std::tuple_element<I, P> { using type = int; };
constexpr P make(const T& t) { return {&t}; }
auto [a, b] = make(T{{3, 4}});
thread_local auto [ta, tb] = make(T{{5, 6}});
constexpr int f() { auto [e, g] = make(T{{7, 8}}); return e * 10 + g; }
static_assert(f() == 78);
int g() { static auto [c, d] = make(T{{1, 2}}); return c * 10 + d; }
int main() { printf("%d %d %d %d %d\n", a, b, ta, tb, g()); }

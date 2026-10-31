// [temp.expl.spec]/1, [temp.deduct.decl]: explicit specializations of
// function templates, with explicit arguments, deduced from the
// declarator, and chosen by partial ordering among overloads.
//@ expect: compile-ok
template <class T> constexpr int f(T) { return 1; }
template <> constexpr int f<int>(int) { return 2; }
template <> constexpr int f(char) { return 3; }
template <class T> constexpr int g(T) { return 10; }
template <class T> constexpr int g(T*) { return 20; }
template <> constexpr int g(int*) { return 30; }
template <> constexpr int g<long>(long) { return 40; }
template <class T, class U = T> constexpr int h() { return 0; }
template <> constexpr int h<int>() { return 5; }
template <> constexpr int h<int, char>() { return 6; }
template <class T> constexpr T zero = T();
template <class T> struct Box { T v; };
template <class T> constexpr int k(Box<T>) { return 1; }
template <> constexpr int k(Box<double>) { return 7; }
namespace n { template <class T> constexpr int m(T) { return 0; } }
template <> constexpr int n::m(bool) { return 8; }
static_assert(f(1.0) == 1 && f(1) == 2 && f('c') == 3);
static_assert(g(1) == 10 && g((char*)nullptr) == 20 && g((int*)nullptr) == 30);
static_assert(g(1L) == 40);
static_assert(h<int>() == 5 && h<int, char>() == 6 && h<char>() == 0);
static_assert(k(Box<int>{}) == 1 && k(Box<double>{}) == 7);
static_assert(n::m(true) == 8 && n::m(1) == 0);

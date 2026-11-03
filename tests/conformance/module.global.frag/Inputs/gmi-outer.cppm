// The importing unit of gmf-merge-importer.cpp: the same global module
// fragment as gmi-inner.cppm, then `export import gmi_inner;` (kiln builds
// gmi_inner in-process after this fragment).
module;
namespace n {
struct S { int v = 4; constexpr int get() const { return v * 2; } };
inline constexpr int g = 7;
template <class T> inline constexpr T z = T(3);
template <> inline constexpr int z<char> = 9;
template <class T> struct W { T t; constexpr T f() { return t + 1; } };
constexpr int h(int x) { return x + 1; }
template <class T> constexpr T ht(T x) { return x; }
enum E { e1 = 5 };
enum class C : int;
inline constexpr bool c_small = sizeof(C) == sizeof(int);
enum class C : int { c0, c1 };
typedef int I;
using J = long;
template <class T> concept K = sizeof(T) > 1;
template <class T> using A = T *;
}
export module gmi_outer;
export import gmi_inner;
export constexpr int outer() { return n::S{}.get() + n::g + n::z<int> + n::z<char> + n::W<int>{5}.f() + n::h(1) + n::ht(2); }

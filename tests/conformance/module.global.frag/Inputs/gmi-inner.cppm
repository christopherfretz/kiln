// The imported unit of gmf-merge-importer.cpp: its global module fragment
// declares the same entities as the importer's (gmi-outer.cppm).
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
export module gmi_inner;
export namespace n {
using n::S; using n::g; using n::z; using n::W; using n::h; using n::ht;
using n::E; using n::e1; using n::C; using n::c_small; using n::I; using n::J;
using n::K; using n::A;
}
export constexpr int inner() { return n::S{}.get() + n::g + n::z<int> + n::z<char> + n::W<int>{5}.f() + n::h(1) + n::ht(2); }

// [lex.key]/1, [diff.cpp03.lex]: alignas, alignof, char16_t, char32_t,
// constexpr, decltype, noexcept, static_assert and thread_local are keywords
// only from C++11 on; in C++98 they are ordinary identifiers. Found running
// GCC's C++ testsuite.
//@ expect: compile-ok
//@ std: c++98
//@ strict-diags

template<bool> struct static_assert;
template<> struct static_assert<true> {};
static_assert<(sizeof(int) > 1)> check;

namespace N {
int alignas = 1, alignof = 2, constexpr = 3, decltype = 4, noexcept = 5;
typedef int char16_t;
typedef long char32_t;
struct thread_local { char16_t a; char32_t b; };
}

int f() { return N::alignas + N::alignof + N::constexpr + N::decltype + N::noexcept; }
int main() { N::thread_local t = {1, 2}; return f() - 15 + t.a - 1; }

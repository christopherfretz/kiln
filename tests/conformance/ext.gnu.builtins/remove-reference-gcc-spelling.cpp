// GCC's __remove_reference(T) is clang's __remove_reference_t(T). As every
// type transform it is one only before '(' (a library's own
// __remove_reference template stays a name), and __has_builtin answers 0, as
// in clang 22 (GCC: 1), so libstdc++ keeps the path it takes today. Found running
// GCC's C++ testsuite. WP-1611.
//@ std: c++20
//@ expect: compile-ok
//@ oracle: gcc
//@ skip(clang): clang 19/22 spell it __remove_reference_t
//@ strict-diags
static_assert(__is_same(__remove_reference(int &), int));
static_assert(__is_same(__remove_reference(const int &&), const int));
static_assert(__is_same(__remove_reference(int *), int *));
template <class T> using RR = __remove_reference(T);
static_assert(__is_same(RR<long &>, long));
namespace lib {
template <class T> struct __remove_reference { using type = T; };
static_assert(__is_same(__remove_reference<int &>::type, int &));
}
#if defined(__clang__) && __has_builtin(__remove_reference)
#error __has_builtin(__remove_reference) is 0 in clang 22
#endif
int main() {}

// [temp.inst]/16, [implimits]: as recursive-satisfaction-depth.cpp at the
// default depth limit (1024): the recursion stops quickly and in bounded
// memory. Found running GCC's C++ testsuite (WP-1456).
//@ std: c++20
//@ expect: compile-fail
//@ mem<=2048
//@ diag: 9 fatal "recursive template instantiation exceeded maximum depth of 1024"
namespace q { struct S {}; }
namespace q { template <int N, class T> concept Deep = requires(T t) { g<N + 1>(t); }; template <int N, class T> requires Deep<N, T> int g(T) { return N; } }
int h() { return g<0>(q::S{}); }

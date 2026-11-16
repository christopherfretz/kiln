// [temp.constr.normal]/1, [temp.constr.order]: before C++26 (P2963) a fold
// expression in a constraint is an atomic constraint; folds over different
// concepts are not identical, so neither declaration is more constrained and
// the call is ambiguous (clang 19 and 22 apply fold expanded constraints only
// in C++26). A fold reached through the same concept is one atom and
// orders. Found running GCC's C++ testsuite.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 22:15 error "call to 'f' is ambiguous"
//@ diag: 18:38 note "candidate function [with Ts = <int, int>]"
//@ diag: 19:37 note "candidate function [with Ts = <int, int>]"
//@ diag: 23:15 error "call to 'g' is ambiguous"
//@ diag: 20:66 note "candidate function [with Ts = <int>]"
//@ diag: 21:65 note "candidate function [with Ts = <int>]"
template <class T> concept Small = sizeof(T) <= 8;
template <class T> concept Tiny = Small<T> && sizeof(T) <= 4;
template <Small... Ts> constexpr int f(Ts...) { return 0; }
template <Tiny... Ts> constexpr int f(Ts...) { return 1; }
template <class... Ts> requires (Small<Ts> && ...) constexpr int g(Ts...) { return 0; }
template <class... Ts> requires (Tiny<Ts> && ...) constexpr int g(Ts...) { return 1; }
static_assert(f(1, 2) == 1, "");
static_assert(g(1) == 1, "");
static_assert(f(1.0) == 0, "");
template <class... Ts> concept AllSmall = (Small<Ts> && ...);
template <class... Ts> requires AllSmall<Ts...> constexpr int h(Ts...) { return 0; }
template <class... Ts> requires AllSmall<Ts...> && (sizeof...(Ts) > 1) constexpr int h(Ts...) { return 1; }
static_assert(h(1, 2) == 1, "");
static_assert(h(1) == 0, "");

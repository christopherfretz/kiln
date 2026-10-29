//@ expect: ast
//@ oracle: clang
// [temp.param]/17: template parameter packs
template <class... Ts> struct A {};
template <int... Ns> struct B {};
template <template <class> class... TTs> struct C {};
template <class T, typename... Rest> struct D {};

//@ expect: ast
//@ oracle: clang
// [temp.param]/4, [temp.concept]: type-constraints on type parameters
template <class T> concept C = true;
template <class T, class U> concept C2 = true;
template <C T> struct A {};
template <C2<int> T> struct B {};
template <C... Ts> struct D {};

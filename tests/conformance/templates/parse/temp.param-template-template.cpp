//@ expect: ast
//@ oracle: clang
// [temp.param]/1: template template parameters with defaults
template <class> struct Box {};
template <template <class> class TT> struct A {};
template <template <class, class...> typename TT, template <class> class U = Box> struct B {};

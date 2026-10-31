// [temp.over.link]/5, [temp.alias], [dcl.typedef]/4: an alias template
// whose type involves an expression on template parameters may be
// redeclared with an equivalent expression (libc++ declares
// `__swap_result_t` in two headers, with different parameter names).
template <bool B, class T = void> struct enable_if {};
template <class T> struct enable_if<true, T> { typedef T type; };
template <bool B, class T = void> using enable_if_t = typename enable_if<B, T>::type;
template <class T> struct is_a { static const bool value = true; };
template <class T> struct is_b { static const bool value = true; };

template <class T>
using swap_result_t = enable_if_t<is_a<T>::value && is_b<T>::value>;
template <class Tp>
using swap_result_t = enable_if_t<is_a<Tp>::value && is_b<Tp>::value>;

template <class T> swap_result_t<T> swap(T &, T &);
template <class U> swap_result_t<U> swap(U &, U &) {}

void use(int &a, int &b) { swap(a, b); }

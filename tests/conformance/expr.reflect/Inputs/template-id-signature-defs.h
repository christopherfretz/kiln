// A specialization of each template-id operand in each signature context
// (WP-1250), defined and explicitly instantiated here; the test's unit
// calls each by its symbol text. `d_`: a decltype return type; `p_`: a
// parameter type through a dependent template argument; `q_`: a
// requires-clause. `at`: an alias template-id whose parameters the
// aliased type determines (`^^AT<T>`); `vt`: a variable template-id;
// `g`: a function template-id; `eq`: a qualified operator function
// template-id; `m`: a member function template-id of a dependent class;
// `nv`: a qualified variable template-id; `f`: an alias template-id with
// a parameter the aliased type does not use (`^^F<T, int>`).
#include "template-id-signature.h"

template <class T> auto d_at(T) -> decltype(K<^^AT<T>>{}) { return {}; }
template <class T> auto d_vt(T) -> decltype(K<^^vt<T>>{}) { return {}; }
template <class T> auto d_g(T) -> decltype(K<^^g<T>>{}) { return {}; }
template <class T> auto d_eq(T) -> decltype(K<^^ns::operator== <T>>{}) { return {}; }
template <class T> auto d_m(T) -> decltype(K<^^S<T>::template m<int>>{}) { return {}; }
template <class T> int p_at(T, K<^^AT<T>>) { return 1; }
template <class T> int p_vt(T, K<^^vt<T>>) { return 1; }
template <class T> int p_g(T, K<^^g<T>>) { return 1; }
template <class T> int p_eq(T, K<^^ns::operator== <T>>) { return 1; }
template <class T> int p_m(T, K<^^S<T>::template m<int>>) { return 1; }
template <class T> int p_nv(T, K<^^ns::nv<T>>) { return 1; }
template <class T> int p_f(T, K<^^F<T, int>>) { return 1; }
template <class T> requires(^^AT<T> != ^^int) int q_at(T) { return 1; }
template <class T> requires(^^vt<T> != ^^int) int q_vt(T) { return 1; }
template <class T> requires(^^g<T> != ^^int) int q_g(T) { return 1; }
template <class T> requires(^^ns::operator== <T> != ^^int) int q_eq(T) { return 1; }
template <class T> requires(^^S<T>::template m<int> != ^^int) int q_m(T) { return 1; }

template K<^^AT<int>> d_at(int);
template K<^^vt<int>> d_vt(int);
template K<^^g<int>> d_g(int);
template K<^^ns::operator== <int>> d_eq(int);
template K<^^S<int>::m<int>> d_m(int);
template int p_at(int, K<^^AT<int>>);
template int p_vt(int, K<^^vt<int>>);
template int p_g(int, K<^^g<int>>);
template int p_eq(int, K<^^ns::operator== <int>>);
template int p_m(int, K<^^S<int>::m<int>>);
template int p_nv(int, K<^^ns::nv<int>>);
template int p_f(int, K<^^F<int, int>>);
template int q_at(int);
template int q_vt(int);
template int q_g(int);
template int q_eq(int);
template int q_m(int);

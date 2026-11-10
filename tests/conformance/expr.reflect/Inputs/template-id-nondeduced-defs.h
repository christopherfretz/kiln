// A specialization of each signature whose dependent reflected alias
// template-id has a parameter in a non-deduced context (WP-1260), defined
// and explicitly instantiated here; the test's unit calls each by its
// symbol text. `d_`: a decltype return type; `p_`: a parameter type
// through a dependent template argument; `q_`: a requires-clause. The
// operand is named after the alias (`d2a`: `D2<T, int>`, `d2b`:
// `D2<int, T>`, `dpa`/`dpb`/`dpc`: `DP<T, int>`/`DP<int, T>`/
// `DP<const int, T>`, `dl`: `DL<T, int>`, `n1`/`n2`/`n3`/`x`/`z`: nested
// aliases, `dv`: a pack expansion of the qualifier).
#include "template-id-nondeduced.h"

template <class T> auto d_d(T) -> decltype(K<^^D<T>>{}) { return {}; }
template <class T> int p_d(T, K<^^D<T>>) { return 1; }
template <class T> int p_dptr(T, K<^^D<T*>>) { return 1; }
template <class T> requires(^^D<T> != ^^int) int q_d(T) { return 1; }
template <class T> int p_d2a(T, K<^^D2<T, int>>) { return 1; }
template <class T> int p_d2b(T, K<^^D2<int, T>>) { return 1; }
template <class T> int p_dpa(T, K<^^DP<T, int>>) { return 1; }
template <class T> int p_dpb(T, K<^^DP<int, T>>) { return 1; }
template <class T> int p_n1(T, K<^^N1<T>>) { return 1; }
template <class T> int p_n2(T, K<^^N2<T>>) { return 1; }
template <class T> int p_n3(T, K<^^N3<T>>) { return 1; }
template <class T> int p_x(T, K<^^X<T>>) { return 1; }
template <class T> int p_z(T, K<^^Z<T>>) { return 1; }
template <class T> int p_dv(T, K<^^DV<T>>) { return 1; }
template <int N> int p_dn(K<^^DN<N>>) { return 1; }
template <class T> int p_dt(T, K<^^DT<T>>) { return 1; }
template <class T> int p_nds(T, K<^^NDS<T>>) { return 1; }
template <class T> int p_nd(T, K<^^ND<T>>) { return 1; }
template <class T> int p_dl(T, K<^^DL<T, int>>) { return 1; }
template <class T> int p_dpc(T, K<^^DP<const int, T>>) { return 1; }

template K<^^D<int>> d_d(int);
template int p_d(int, K<^^D<int>>);
template int p_dptr(int, K<^^D<int*>>);
template int q_d(int);
template int p_d2a(int, K<^^D2<int, int>>);
template int p_d2b(int, K<^^D2<int, int>>);
template int p_dpa(int, K<^^DP<int, int>>);
template int p_dpb(int, K<^^DP<int, int>>);
template int p_n1(int, K<^^N1<int>>);
template int p_n2(int, K<^^N2<int>>);
template int p_n3(int, K<^^N3<int>>);
template int p_x(int, K<^^X<int>>);
template int p_z(int, K<^^Z<int>>);
template int p_dv(int, K<^^DV<int>>);
template int p_dn<3>(K<^^DN<3>>);
template int p_dt(int, K<^^DT<int>>);
template int p_nds(int, K<^^NDS<int>>);
template int p_nd(int, K<^^ND<int>>);
template int p_dl(int, K<^^DL<int, int>>);
template int p_dpc(int, K<^^DP<const int, int>>);

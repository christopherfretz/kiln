// [temp.type]/1, [temp.over.link]/5: non-expansion non-type arguments of a
// dependent class template-id with a parameter pack or pack expansion are
// converted when their parameter's type is known (before the first expansion
// and inside a trailing parameter pack, including `auto...`, structural class
// NTTPs, enumerators, and defaults before a pack). Arguments after an
// expansion that hits a non-pack parameter, and arguments whose parameter type
// is still dependent (`template<class U, U...>`), remain unconverted
// (WP-3689).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17
extern "C" int printf(const char *, ...);

namespace n {
constexpr int k = 3;
}

enum E { e0 = 0, e = 2 };
struct S {
  int x;
};
constexpr S s{1};

template <int, int...> struct V {};
template <int, int, int...> struct V2 {};
template <int, int> struct P2 {};
template <class T, int... Ns> struct W {
  static W *self();
};
template <class T, int... Ns> W<T, Ns...> *W<T, Ns...>::self() { return nullptr; }
template <class T> struct W<T, 3> {
  static W *make();
  int tag() const { return 15; }
};
template <class T> W<T, 3> *W<T, 1 + 2>::make() { return nullptr; }

template <class T, unsigned... Ns> struct WU {};
template <class T, E... Es> struct WE {};
template <class T, S... Ss> struct WS {};
template <class T, auto... Ns> struct WA {};
template <class U, U... vs> struct WD {};
template <class T, int = 1, int...> struct D2 {};

// Redeclarations spelling equivalent converted arguments differently.
template <int... Ns> int f_v(V<n::k, Ns...>);
template <int... Ns> int f_v(V<3, Ns...>) { return 1; }

template <int... Ns> int f_v_cond(V<1 == 1 ? 3 : 0, Ns...>);
template <int... Ns> int f_v_cond(V<3, Ns...>) { return 2; }

template <class T> int f_w(W<T, n::k>);
template <class T> int f_w(W<T, 3>) { return 3; }

template <class T> int f_wu(WU<T, 2>);
template <class T> int f_wu(WU<T, 2u>) { return 4; }

template <class T> int f_we_int(W<T, e>);
template <class T> int f_we_int(W<T, 2>) { return 5; }

template <class T> int f_we(WE<T, e>);
template <class T> int f_we(WE<T, (E)2>) { return 6; }

template <class T, int... Ns> int f_w_after(W<T, Ns..., n::k>);
template <class T, int... Ns> int f_w_after(W<T, Ns..., 3>) { return 7; }

template <class T, unsigned... Ns> int f_wu_after(WU<T, Ns..., 2>);
template <class T, unsigned... Ns> int f_wu_after(WU<T, Ns..., 2u>) { return 8; }

template <int... Ns> int f_v2_before(V2<n::k, Ns..., 4>);
template <int... Ns> int f_v2_before(V2<3, Ns..., 4>) { return 9; }

template <class T> int f_ws(WS<T, S{1}>);
template <class T> int f_ws(WS<T, s>);
template <class T> int f_ws(WS<T, {1}>) { return 10; }

template <class T> int f_wa(WA<T, n::k>);
template <class T> int f_wa(WA<T, 3>) { return 11; }

template <class T, int... Ns> int f_wa_after(WA<T, Ns..., n::k>);
template <class T, int... Ns> int f_wa_after(WA<T, Ns..., 3>) { return 12; }

template <class T> int f_d2(D2<T>);
template <class T> int f_d2(D2<T, 1>) { return 13; }

template <int N> int f_wd_known(WD<int, N, 1>);
template <int N> int f_wd_known(WD<int, N, 1u>) { return 14; }

// Distinct overloads: not redefinitions.
template <int... Ns> int dist_p2(P2<Ns..., n::k>) { return 100; }
template <int... Ns> int dist_p2(P2<Ns..., 3>) { return 101; }

template <int... Ns> int dist_v2(V2<3, Ns..., 1 + 1>) { return 102; }
template <int... Ns> int dist_v2(V2<3, Ns..., 2>) { return 103; }

template <class T> int dist_wd(WD<T, 1>) { return 104; }
template <class T> int dist_wd(WD<T, 1u>) { return 105; }

template <class T> int dist_wa(WA<T, 1>) { return 16; }
template <class T> int dist_wa(WA<T, 1u>) { return 17; }

int main() {
  int r1 = f_v<1, 2>(V<3, 1, 2>{});
  int r2 = f_v_cond<4>(V<3, 4>{});
  int r3 = f_w<int>(W<int, 3>{});
  int r4 = f_wu<int>(WU<int, 2u>{});
  int r5 = f_we_int<int>(W<int, 2>{});
  int r6 = f_we<int>(WE<int, e>{});
  int r7 = f_w_after<int, 1>(W<int, 1, 3>{});
  int r8 = f_wu_after<int, 1u>(WU<int, 1u, 2u>{});
  int r9 = f_v2_before<1>(V2<3, 1, 4>{});
  int r10 = f_ws<int>(WS<int, S{1}>{});
  int r11 = f_wa<int>(WA<int, 3>{});
  int r12 = f_wa_after<int, 1>(WA<int, 1, 3>{});
  int r13 = f_d2<int>(D2<int, 1>{});
  int r14 = f_wd_known<5>(WD<int, 5, 1>{});
  int r15 = W<int, 3>::make() == nullptr && W<int, 1, 2>::self() == nullptr ? W<int, 3>{}.tag() : 0;
  int r16 = dist_wa<int>(WA<int, 1>{});
  int r17 = dist_wa<int>(WA<int, 1u>{});
  printf("%d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d %d\n", r1, r2, r3, r4, r5, r6, r7, r8,
         r9, r10, r11, r12, r13, r14, r15, r16, r17);
}

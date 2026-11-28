// [temp.type]/1, [temp.over.link]/5: defining both functions of an equivalent
// dependent template-id pair with packs is a redefinition (WP-3689).
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 42:27 error "redefinition of 'f'"
//@ diag: 41:27 note "previous definition is here"
//@ diag: 45:25 error "redefinition of 'g'"
//@ diag: 44:25 note "previous definition is here"
//@ diag: 48:25 error "redefinition of 'h'"
//@ diag: 47:25 note "previous definition is here"
//@ diag: 51:25 error "redefinition of 'j'"
//@ diag: 50:25 note "previous definition is here"
//@ diag: 54:36 error "redefinition of 'm'"
//@ diag: 53:36 note "previous definition is here"
//@ diag: 57:27 error "redefinition of 'p'"
//@ diag: 56:27 note "previous definition is here"
//@ diag: 60:25 error "redefinition of 'q'"
//@ diag: 59:25 note "previous definition is here"
//@ diag: 63:25 error "redefinition of 'r'"
//@ diag: 62:25 note "previous definition is here"
//@ diag: 66:25 error "redefinition of 'u'"
//@ diag: 65:25 note "previous definition is here"
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
template <class T, int... Ns> struct W {};
template <class T, unsigned... Ns> struct WU {};
template <class T, S... Ss> struct WS {};
template <class T, auto... Ns> struct WA {};
template <class T, int = 1, int...> struct D2 {};

template <int... Ns> void f(V<n::k, Ns...>) {}
template <int... Ns> void f(V<3, Ns...>) {}

template <class T> void g(W<T, n::k>) {}
template <class T> void g(W<T, 3>) {}

template <class T> void h(WU<T, 2>) {}
template <class T> void h(WU<T, 2u>) {}

template <class T> void j(W<T, e>) {}
template <class T> void j(W<T, 2>) {}

template <class T, int... Ns> void m(W<T, Ns..., n::k>) {}
template <class T, int... Ns> void m(W<T, Ns..., 3>) {}

template <int... Ns> void p(V2<n::k, Ns..., 4>) {}
template <int... Ns> void p(V2<3, Ns..., 4>) {}

template <class T> void q(WS<T, S{1}>) {}
template <class T> void q(WS<T, {1}>) {}

template <class T> void r(D2<T>) {}
template <class T> void r(D2<T, 1>) {}

template <class T> void u(WA<T, n::k>) {}
template <class T> void u(WA<T, 3>) {}

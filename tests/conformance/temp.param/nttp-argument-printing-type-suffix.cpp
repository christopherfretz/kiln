// [temp.param]/6.3, [temp.arg.nontype]/1, [lex.icon]: diagnostics print a
// template argument of a non-type parameter with a placeholder type with its
// type (clang `TemplateParameterList::shouldIncludeTypeForArgument`:
// `D<7UL>`, `Mix<int, 4LL>`, `P<1, 2U, 'c'>`), and an argument expression
// as written with its integer-literal suffix; an argument of a parameter of
// known type prints bare (`A<5>`).
//@ verify
template <auto N> struct D {};
template <auto... V> struct P {};
template <class T, auto N> struct Mix {};
template <unsigned long N> struct A {};
template <class T> struct X {};

void f(D<7ul>* d, P<1, 2u, 'c'> p, Mix<int, 4ll> m, A<5> a, X<D<3>> x) {
  int i1 = d; // expected-error {{cannot initialize a variable of type 'int' with an lvalue of type 'D<7UL> *'}}
  int i2 = p; // expected-error {{no viable conversion from 'P<1, 2U, 'c'>' to 'int'}}
  int i3 = m; // expected-error {{no viable conversion from 'Mix<int, 4LL>' to 'int'}}
  int i4 = a; // expected-error {{no viable conversion from 'A<5>' to 'int'}}
  int i5 = x; // expected-error {{no viable conversion from 'X<D<3>>' to 'int'}}
}

template <auto N> struct S { static_assert(N == 0); }; // expected-error {{static assertion failed due to requirement '5UL == 0'}}
S<5ul> s; // expected-note {{in instantiation of template class 'S<5UL>' requested here}}

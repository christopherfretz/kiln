// [temp.arg.nontype]/1 with [expr.prim.id.general]/4: a non-static data
// member named without an object in a non-type template argument is
// erroneous; only that error is reported, with no "not a constant
// expression" after it.
//@ verify
template <class T, T v> struct IC { static constexpr T value = v; };
template <class T> struct W : IC<unsigned long, T::value> {}; // expected-error {{invalid use of non-static data member 'value'}}
struct E { unsigned long value; };
struct G { static const unsigned long value = 2; };
static_assert(W<G>::value == 2);
void f() {
  (void)W<E>::value; // expected-note {{in instantiation of template class 'W<E>' requested here}}
}
struct S {
  int m;
  IC<int, S::m> *p; // expected-error {{invalid use of non-static data member 'm'}}
};

// [temp.arg.nontype]/1, [temp.param]/8: a class or floating-point value
// argument in a specialization's name, as clang's `TemplateArgument::print`
// spells it (`APValue::printPretty`): only the value's class is named, a
// member subobject is a braced list (`Out{{2}, ...}`); a floating-point
// value is its `double` in `%e` form (`2.500000e-01`), an infinity `INF`.
//@ verify
struct In { int i; };
struct Out { In in; double d; };
template <auto V> struct T {};
int a = T<1.5>::x; // expected-error {{no member named 'x' in 'T<1.500000e+00>'}}
int b = T<Out{{2}, 0.25}>::x; // expected-error {{{no member named 'x' in 'T<Out{{2}, 2.500000e-01}>'}}}
int c = T<__builtin_inf()>::x; // expected-error {{no member named 'x' in 'T<INF>'}}
int d = T<-0.0f>::x; // expected-error {{no member named 'x' in 'T<-0.000000e+00>'}}

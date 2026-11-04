// [dcl.type.auto.deduct]/2.1-2.2: a placeholder variable's parenthesized or
// direct-list initializer has exactly one expression, which is not itself a
// braced-init-list; both are checked in a template definition, before the
// initializer's type is known (a pack expansion counts as one expression).
//@ verify
void f() {
  auto a({1}); // expected-error {{cannot deduce type for variable 'a' with type 'auto' from parenthesized initializer list}}
  auto b{{1}}; // expected-error {{cannot deduce type for variable 'b' with type 'auto' from nested initializer list}}
  auto c(1, 2); // expected-error {{initializer for variable 'c' with type 'auto' contains multiple expressions}}
}
template <class T, class... U> void g(T t, U... u) {
  auto a({t}); // expected-error {{cannot deduce type for variable 'a' with type 'auto' from parenthesized initializer list}}
  auto b{{t}}; // expected-error {{cannot deduce type for variable 'b' with type 'auto' from nested initializer list}}
  auto c(t, t); // expected-error {{initializer for variable 'c' with type 'auto' contains multiple expressions}}
  auto d{t, t}; // expected-error {{initializer for variable 'd' with type 'auto' contains multiple expressions}}
  auto e(u...);
  auto h{u...};
}

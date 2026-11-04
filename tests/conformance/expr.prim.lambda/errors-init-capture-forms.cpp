// [expr.prim.lambda.capture]/6, [dcl.type.auto.deduct]: an init-capture is
// deduced as `auto name initializer`, so a braced or parenthesized
// initializer needs exactly one expression that is not itself braced
// (checked in a template definition too), `= {...}` needs
// <initializer_list>, an overload set deduces nothing, and
// copy-initialization does not call an explicit constructor.
//@ verify
struct E { E(int); explicit E(const E &); }; // expected-note {{candidate constructor not viable: no known conversion from 'const E' to 'int' for 1st argument}} expected-note {{explicit constructor is not a candidate}}
void o(int);
void o(char);
void f(int a) {
  auto b = [x{1, 2}] {}; // expected-error {{initializer for lambda capture 'x' contains multiple expressions}}
  auto c = [x(a, a)] {}; // expected-error {{initializer for lambda capture 'x' contains multiple expressions}}
  auto d = [x()] {}; // expected-error {{expected expression}}
  auto e = [x = {1}] {}; // expected-error {{cannot deduce type of initializer list because std::initializer_list was not found; include <initializer_list>}}
  auto h = [x{}] {}; // expected-error {{initializer missing for lambda capture 'x'}}
  auto i = [x({1})] {}; // expected-error {{cannot deduce type for lambda capture 'x' from parenthesized initializer list}}
  auto j = [x{{1}}] {}; // expected-error {{cannot deduce type for lambda capture 'x' from nested initializer list}}
  auto k = [x(o)] {}; // expected-error {{cannot deduce type for lambda capture 'x' from initializer of type '<overloaded function type>'}}
  auto ok = [x(a), y{a}] {};
  const E ce(1);
  auto g = [q = ce] {}; // expected-error {{no matching constructor for initialization of 'E'}}
}
template <class T> void t(T v) {
  auto b = [x{v, v}] {}; // expected-error {{initializer for lambda capture 'x' contains multiple expressions}}
  auto c = [x(v, v)] {}; // expected-error {{initializer for lambda capture 'x' contains multiple expressions}}
  auto i = [x({v})] {}; // expected-error {{cannot deduce type for lambda capture 'x' from parenthesized initializer list}}
}

// [temp.local]/6: an enumerator, of a scoped or unscoped enumeration,
// cannot have the name of a template parameter in scope.
//@ verify

template <int I> struct Tm { enum { I }; };          // expected-error {{declaration of 'I' shadows template parameter}} expected-note {{template parameter is declared here}}
template <class T> struct Tc { enum class E { T }; }; // expected-error {{declaration of 'T' shadows template parameter}} expected-note {{template parameter is declared here}}
template <class U> void f() { enum { V, U }; }       // expected-error {{declaration of 'U' shadows template parameter}} expected-note {{template parameter is declared here}}
template <int N> struct Ok { enum { M = N }; };
Ok<1> ok;

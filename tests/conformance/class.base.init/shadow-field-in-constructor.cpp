// [class.base.init]: a constructor parameter that hides a field warns at
// the end of the constructor under -Wshadow-field-in-constructor (in
// -Wshadow-all), unless the body modifies it, which warns under
// -Wshadow-field-in-constructor-modified (in -Wshadow) at the
// modification instead.
//@ verify
//@ args: -Wshadow-all

struct B { int bf; protected: int bp; private: int bq; }; // expected-note {{declared here}} expected-note {{declared here}} expected-note {{declared here}}
struct S : B {
  int f; int bf; int bp; int bq; // expected-warning {{non-static data member 'bf' of 'S' shadows member inherited from type 'B'}} expected-warning {{non-static data member 'bp' of 'S' shadows member inherited from type 'B'}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}} expected-note {{previous declaration is here}}
  S(int f, int g) : f(f) { (void)g; } // expected-warning {{constructor parameter 'f' shadows the field 'f' of 'S'}}
  S(int f) { f = 2; f++; } // expected-warning {{modifying constructor parameter 'f' that shadows a field of 'S'}} expected-note {{variable 'f' is declared here}}
  S(long f, int x, int y) { (void)f; (void)x; (void)y; } // expected-warning {{constructor parameter 'f' shadows the field 'f' of 'S'}}
  S(char f, char bf) : f(f), bf(bf) {} // expected-warning {{parameter 'bf' shadows member inherited from type 'B'}} expected-warning {{constructor parameter 'f' shadows the field 'f' of 'S'}} expected-warning {{constructor parameter 'bf' shadows the field 'bf' of 'S'}}
  S(short f) { f += 1; } // expected-warning {{modifying constructor parameter 'f' that shadows a field of 'S'}} expected-note {{variable 'f' is declared here}}
};
template <class T> struct TS { T v; TS(T v) : v(v) {} }; // expected-warning {{constructor parameter 'v' shadows the field 'v' of 'TS'}} expected-note {{previous declaration is here}}
TS<int> ti(1); TS<long> tl(2);

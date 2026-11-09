// [class.base.init]: -Wshadow enables only the modified flavour of the
// constructor-parameter warning; -Wno-shadow-field-in-constructor-modified
// turns it off again.
//@ verify
//@ args: -Wshadow

struct S {
  int f; // expected-note {{previous declaration is here}}
  S(int f, int g) : f(f) { (void)g; }
  S(int f) { f = 2; } // expected-warning {{modifying constructor parameter 'f' that shadows a field of 'S'}} expected-note {{variable 'f' is declared here}}
};

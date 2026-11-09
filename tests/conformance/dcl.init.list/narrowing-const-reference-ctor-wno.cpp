// [dcl.init.list]/3.7: -Wno-c++11-narrowing-const-reference silences the
// narrowing of a constructor's reference parameter.
//@ verify
//@ args: -Wno-c++11-narrowing-const-reference
// expected-no-diagnostics

struct Q { Q(const signed char &, int); };
unsigned char u();
void g() { Q q{u(), 1}; (void)q; }

// [class.base.init]: -Wno-shadow-field-in-constructor-modified after
// -Wshadow silences the modification warning.
//@ verify
//@ args: -Wshadow -Wno-shadow-field-in-constructor-modified

// expected-no-diagnostics
struct S {
  int f;
  S(int f) { f = 2; }
};

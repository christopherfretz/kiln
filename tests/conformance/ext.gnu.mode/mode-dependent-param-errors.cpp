//@ verify
// GCC mode on a parameter of dependent type (WP-809): a mode that does not
// fit the substituted type is a substitution failure (clang
// `instantiateDependentModeAttr`).
template <class T> int f(T x __attribute__((mode(DI)))) { return sizeof(x); }
// expected-note@-1 {{candidate template ignored: substitution failure [with T = float]: type of machine mode does not match type of base type}}
int a = f<float>(1); // expected-error {{no matching function for call to 'f'}}
int b = f<short>(1);

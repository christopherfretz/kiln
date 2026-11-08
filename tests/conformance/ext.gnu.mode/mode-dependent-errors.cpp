//@ verify
// A mode that does not fit the substituted type is diagnosed at
// instantiation (clang `instantiateDependentModeAttr`).
template <class T> struct Bad { typedef T t __attribute__((mode(SF))); };
// expected-error@-1 {{type of machine mode does not match type of base type}}
Bad<int>::t bad; // expected-note {{in instantiation of template class 'Bad<int>' requested here}}

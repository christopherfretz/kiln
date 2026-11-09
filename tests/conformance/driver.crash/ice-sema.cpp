// An internal compiler error in end-of-translation-unit semantic analysis.
// spec/driver.md "Internal compiler errors", WP-911.
//@ expect: ice
//@ args: -fkiln-crash-for-testing=sema
//@ ice: internal compiler error: crash requested by -fkiln-crash-for-testing=sema
//@ ice: ice-sema.cpp' (phase: sema)
//@ ice: .ii
//@ ice: .sh

template <class T> T id(T t) { return t; }
int x = id(1);

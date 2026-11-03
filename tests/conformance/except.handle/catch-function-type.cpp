// [except.pre]/1, [except.handle]/2: an exception-declaration may be an
// abstract declarator, so `catch (int(...))` is a handler of type "function
// of (...) returning int", adjusted to "pointer to function" and matching
// a thrown `int (*)(...)`, also when the type is a template parameter's.
//@ std: c++26
//@ expect: run
//@ strict-diags
template<class T> int k() {
  try { throw (T(*)(...))nullptr; } catch (T(...)) { return 0; }
  return 1;
}
int main() {
  try { throw (int(*)(...))nullptr; } catch (int(...)) { return k<int>(); }
  return 1;
}

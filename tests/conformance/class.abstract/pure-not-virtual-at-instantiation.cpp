// [class.abstract]/2: a pure-specifier on a member of a class template
// that overrides nothing is diagnosed when the class is instantiated
// (clang `CheckPureMethod` from the instantiation), not at the template
// definition, since a dependent base could have made it virtual. WP-726.
//@ std: c++20
//@ expect: compile-fail
//@ strict-diags
//@ diag: 12:8 error "'w' is not virtual and cannot be declared pure"
//@ diag: 14:21 note "in instantiation of template class 'W<N>' requested here"
struct N { void w(int); };
template <class T> struct W : T {
  void w(int) = 0;
};
int main() { return sizeof(W<N>) == 1; }

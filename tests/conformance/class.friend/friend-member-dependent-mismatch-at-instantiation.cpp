// [class.friend]/10, [temp.friend]/1: a friend naming a member function of
// a non-template class with a type that depends on the enclosing class
// template is matched when the class is instantiated (WP-1110), and a
// specialization whose substituted type matches no member is diagnosed
// then (clang adds a candidate note kiln does not emit).
//@ expect: compile-fail
//@ diag: 13:17 error "friend declaration of 'f' does not match any declaration in 'B'"
//@ diag: 16:9 note "in instantiation of template class 'W<char>' requested here"
template <class T> class W;
struct B { static int f(W<int> &w); };
template <class T> class W {
  int secret = 42;
  friend int B::f(W<T> &);
};
W<int> a;
W<char> c;

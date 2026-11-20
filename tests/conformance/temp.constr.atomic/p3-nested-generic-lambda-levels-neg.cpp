// [temp.constr.atomic]/3, [temp.constr.decl]/3: the constraints of a lambda
// in a generic lambda in a template are checked with the arguments of every
// enclosing level; an unsatisfied one makes the call ill-formed.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 20:9 error "no matching function for call to object of type '(lambda at"
//@ diag: 17:10 note "candidate template ignored: constraints not satisfied [with V = char]"
//@ diag: 17:52 note "because 'char' does not satisfy 'C'"
//@ diag: 14:32 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
//@ diag: 24:13 error "invalid reference to function 'g': constraints not satisfied"
//@ diag: 26:20 note "in instantiation of function template specialization"
//@ diag: 23:46 note "because 'char' does not satisfy 'C'"
//@ diag: 14:32 note "because 'sizeof(char) > 1' (1 > 1) evaluated to false"
template <class T> concept C = sizeof(T) > 1;
template <class T>
auto a = [] <class U> (U) {
  return [] <class V> (V) requires C<T> && C<U> && C<V> { return 1; };
};
int x = a<int>(0)(0);
int y = a<int>(0)('c');
template <class T>
auto local = [] <class U> (U) {
  struct L { static int g() requires C<U> && C<T> { return 6; } };
  return L::g();
};
int z = local<char>(0);

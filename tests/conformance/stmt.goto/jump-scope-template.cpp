// [stmt.dcl]/3, [temp.inst]: the jump-scope check runs on each
// instantiation; a goto past a variable of a class type with a non-trivial
// destructor is ill-formed only in that specialization.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 11:10 error "cannot jump from this goto statement to its label"
//@ diag: 17:15 note "in instantiation of function template specialization 'bad<D>' requested here"
//@ diag: 12:5 note "jump bypasses variable with a non-trivial destructor"
struct D { int n; ~D(); };
template <class T> void bad(bool b) {
  if (b) goto l;
  T t;
l:;
}
template void bad<int>(bool);
// The specialization with D is ill-formed.
template void bad<D>(bool);

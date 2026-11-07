// [dcl.fct.default]/5, [temp.dep.type]/7: the default argument of a
// parameter whose type is a nested class of a class template is checked
// when it is instantiated for a call, not in the template definition.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 12:66 error "no viable conversion from 'int' to 'H'"
//@ diag: 16:10 note "in instantiation of default function argument expression for 'q<int>' required here"
//@ diag: 12:37 note "candidate constructor (the implicit copy constructor) not viable"
//@ diag: 12:37 note "candidate constructor (the implicit move constructor) not viable"
//@ diag: 12:66 note "passing argument to parameter 'o' here"
template<class S> struct L { struct H { int v; }; static int q(H o = 5) { return o.v; } };
int ok() { return sizeof(L<char>); }  // the class alone: no error
int main() {
  L<int> a;
  return L<int>::q();
}


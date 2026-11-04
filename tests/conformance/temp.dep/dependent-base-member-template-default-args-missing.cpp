// [temp.dep.type]/7, [dcl.fct.default]/1, [expr.call]/7: a static member
// function found at instantiation through a dependent base by
// `this->f(...)` has no default argument for a parameter declared without
// one, so a call that omits it has too few arguments: a member template
// is not viable, a non-template is called with too few arguments.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 19:32 error "no matching member function for call to 'pick'"
//@ diag: 22:42 note "in instantiation of member function 'Use<int>::run' requested here"
//@ diag: 15:33 note "candidate function template not viable: requires 2 arguments, but 1 was provided"
//@ diag: 20:37 error "too few arguments to function call, expected 2, have 1"
//@ diag: 22:54 note "in instantiation of member function 'Use<int>::add_one' requested here"
//@ diag: 16:14 note "'add' declared here"
template <class T> struct Base {
  template <class U> static int pick(U* p, int k) { return k; }
  static int add(int a, int b) { return a + b; }
};
template <class T> struct Use : Base<T> {
  int run(T* p) { return this->pick(p); }
  int add_one() { return this->add(1); }
};
int main() { Use<int> u; int x; return u.run(&x) + u.add_one(); }

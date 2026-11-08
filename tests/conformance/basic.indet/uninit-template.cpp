// [temp.inst], [basic.indet]: clang analyzes each function template
// specialization once, when it is instantiated, and its warnings carry the
// instantiation note (a second use of never<int> adds nothing); member
// functions and lambda bodies are analyzed too.
//@ expect: compile-ok
//@ args: -Wuninitialized
//@ strict-diags
//@ diag: 29:7 warning "variable 'x' is used uninitialized whenever 'if' condition is false"
//@ diag: 33:14 note "in instantiation of function template specialization 'maybe<int>' requested here"
//@ diag: 31:10 note "uninitialized use occurs here"
//@ diag: 29:3 note "remove the 'if' if its condition is always true"
//@ diag: 28:6 note "initialize the variable 'x' to silence this warning"
//@ diag: 54:9 warning "variable 'z' is used uninitialized whenever 'if' condition is false"
//@ diag: 56:12 note "uninitialized use occurs here"
//@ diag: 54:5 note "remove the 'if' if its condition is always true"
//@ diag: 53:10 note "initialize the variable 'z' to silence this warning"
//@ diag: 62:7 warning "variable 'q' is used uninitialized whenever 'if' condition is false"
//@ diag: 64:10 note "uninitialized use occurs here"
//@ diag: 62:3 note "remove the 'if' if its condition is always true"
//@ diag: 61:8 note "initialize the variable 'q' to silence this warning"
//@ diag: 37:10 warning "variable 'y' is uninitialized when used here"
//@ diag: 40:3 note "in instantiation of function template specialization 'never<int>' requested here"
//@ diag: 36:6 note "initialize the variable 'y' to silence this warning"
//@ diag: 37:10 warning "variable 'y' is uninitialized when used here"
//@ diag: 41:3 note "in instantiation of function template specialization 'never<long>' requested here"
//@ diag: 36:6 note "initialize the variable 'y' to silence this warning"
template <class T> T maybe(bool b) {
  T x;
  if (b)
    x = 1;
  return x;
}
template int maybe<int>(bool);

template <class T> T never() {
  T y;
  return y;
}
void use() {
  never<int>();
  never<long>();
  never<int>();
}

template <class T> T dependent_init() {
  T z = T();
  return z;
}
template int dependent_init<int>();

struct S {
  int method(bool b) {
    int z;
    if (b)
      z = 2;
    return z;
  }
};

auto lam = [](bool b) {
  int q;
  if (b)
    q = 1;
  return q;
};
int call_lam() { return lam(true); }

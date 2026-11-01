// [dcl.ambig.res]/2, [dcl.init.list]: `(T()){}` is a compound literal
// (a `{` after an ambiguous parenthesized type-id) of the function type
// `T ()`, which cannot be list-initialized: clang's InitListChecker says
// `illegal initializer type` at the `{`. A dependent function type is
// diagnosed at instantiation.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 18:8 error "illegal initializer type 'T ()'"
//@ diag: 19:8 error "illegal initializer type 'T ()'"
//@ diag: 20:6 error "illegal initializer type 'F' (aka 'int ()')"
//@ diag: 21:21 error "illegal initializer type 'int (int)'"
//@ diag: 24:35 error "illegal initializer type 'int ()'"
//@ diag: 25:15 note "in instantiation of function template specialization 'h<int>' requested here"
//@ diag: 29:12 warning "expression result unused"
struct T { T(); T(int); };
typedef int F();
void g() {
  (T()){};
  (T()){1, 2};
  (F){1};
  int x = (int(int)){} + 1;
  (void)x;
}
template<class U> void h() { (U()){}; }
template void h<int>();
template<class U> int k() { return (U){1}; }
int y = k<int>();
template<class U> void m() { (U()){}; }
void n() { (void (*)()){}; }

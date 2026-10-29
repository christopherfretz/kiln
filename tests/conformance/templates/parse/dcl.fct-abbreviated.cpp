//@ expect: ast
//@ oracle: clang
// [dcl.fct]/22: abbreviated function templates (invented parameters)
template <class T> concept C = true;
void f(auto x);
void g(C auto x, auto *p);
void h(auto... xs);
int k(const auto &r) { return 0; }

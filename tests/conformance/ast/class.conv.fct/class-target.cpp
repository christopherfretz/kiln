//@ expect: ast
//@ oracle: clang
// [dcl.init.general]/16.6.2, CWG2327: initialization of a class from
// a class with a conversion function: the bare user-defined conversion
// in direct-initialization, through the copy/move constructor in
// copy-initialization, and in argument passing and return.
struct B { int v; };
struct P { operator B(); };
struct R { operator B&(); };
void take(B);
void takec(const B&);
void f(P p, R r) {
  B a = static_cast<B>(p);
  B c = p;
  B d(p);
  B e = (B)r;
  take(p);
  takec(p);
  takec(r);
}
B ret(P p) { return p; }

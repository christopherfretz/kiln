// [expr.new]/19, [expr.delete]/12, [temp.constr.atomic]/3: an
// inaccessible class-scope operator new / operator delete makes the
// new- or delete-expression invalid, so a requires-expression using it is
// not satisfied; a friend may use them (libc++ shared_ptr_Y.pass.cpp).
//@ expect: compile-ok
//@ strict-diags
class P { void *operator new(__SIZE_TYPE__); void operator delete(void *); };
struct Q {};
template <class T> concept newable = requires { new T; };
template <class T> concept deletable = requires(T *p) { delete p; };
static_assert(!newable<P>);
static_assert(newable<Q>);
static_assert(!deletable<P>);
static_assert(deletable<Q>);
class F {
  friend void f();
  void *operator new(__SIZE_TYPE__);
  void operator delete(void *);
};
void f() {
  F *p = new F;
  delete p;
}

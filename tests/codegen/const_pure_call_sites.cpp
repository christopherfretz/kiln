// `const` / `pure` callees (WP-182): clang 19 `ConstructAttributeList` puts
// `nounwind willreturn memory(none)` / `memory(read)` on the declaration
// and on every call site, member calls included (a virtual call takes the
// named method's attributes); a return or argument passed by address
// (`sret`, `byval`, a non-trivial class by value) widens it with
// `argmem: readwrite`; a function template specialization or a member of
// a class template specialization has its pattern's attributes.
struct S {
  __attribute__((pure)) int m() const;
  __attribute__((const)) int c() const;
  __attribute__((const)) static int s(int);
  __attribute__((pure)) bool operator==(const S&) const;
  __attribute__((pure)) virtual int v() const;
  int k() const { return m() + c(); }
};
__attribute__((const)) int operator+(S, int);
template<class T> __attribute__((pure)) T tp(T);
template<class T> [[noreturn]] void nr(T);

struct B { long a[5]; };
__attribute__((const)) int byval_arg(B);
__attribute__((pure)) int byval_arg_pure(B);
__attribute__((const)) B sret_ret(int);
__attribute__((pure)) B sret_ret_pure(int);

template<class T> struct X {
  __attribute__((pure)) T get() const;
  template<class U> __attribute__((const)) U cv(U) const;
};

int use(S& s, S* ps, B b, X<long>& x) {
  return s.m() + ps->c() + s.s(4) + (s == *ps) + s.v() + (s + 1) + tp(2) + s.k() +
         byval_arg(b) + byval_arg_pure(b) + sret_ret(1).a[0] + sret_ret_pure(2).a[0] +
         x.get() + x.cv(3);
}
void die() { nr(1); }

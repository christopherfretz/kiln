// [temp.explicit]/4, /10, [temp.deduct.decl], [temp.mem]/6: explicit
// instantiation definitions naming member function template
// specializations of a class by signature, without template arguments:
// a constructor template, a conversion function template (every one is
// considered, [temp.mem]/6), a non-static member template with a
// defaulted non-type parameter (deduced from its default), a static
// member template, a constructor template of a class template
// specialization, and a free function template (with `<>`, explicit
// arguments, or none). Each is defined here and used. WP-580.
//@ expect: run
//@ strict-diags
//@ stdout: 7 0 13 5 9 4 6 8
extern "C" int printf(const char *, ...);

struct A {
  template <class T> A(T t);
  template <class T> operator T *() const;
  template <class T, int N = 3> int get(T) const;
  template <class T> static int make(T);
  int v;
};
template <class T> A::A(T t) : v(int(t)) {}
template <class T> A::operator T *() const { return nullptr; }
template <class T, int N> int A::get(T t) const { return v + int(t) + N; }
template <class T> int A::make(T t) { return int(t) + 1; }

template A::A(long);
template A::operator char *() const;
template int A::get(int) const;
template int A::make(short);

template <class C> struct B {
  template <class U> B(U u);
  int v;
};
template <class C> template <class U> B<C>::B(U u) : v(int(u) * 3) {}
template B<int>::B(char);

template <class T> T twice(T t) { return t + t; }
template int twice(int);
template long twice<>(long);
template short twice<short>(short);

int main() {
  A a(7L);
  char *p = a;
  B<int> b(char(3));
  printf("%d %d %d %d %d %d %ld %d\n", a.v, p != nullptr, a.get(3), A::make(short(4)), b.v,
         twice(2), twice(3L), int(twice(short(4))));
}

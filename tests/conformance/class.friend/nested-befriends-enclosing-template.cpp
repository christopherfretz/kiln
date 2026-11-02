// [class.friend]/2, [temp.local]/1, [temp.dep.type]/1: in a class nested in
// a class template, `friend class D;` names the injected-class-name, i.e.
// the current instantiation D<T>, so each specialization's members --
// including a member template defined out of class and instantiated later
// -- may use the nested class's private members (libc++
// binomial_distribution::param_type). `friend class A<T>;` and `friend C;`
// name the same class, and so does `class S` in a partial specialization.
//@ expect: run
//@ stdout: 7 7 6 6 9 5
extern "C" int printf(const char *, ...);
template <class T> class D {
public:
  class P {
    T t_;
  public:
    P(T t) : t_(t) {}
    friend class D;
  };
  template <class G> T operator()(G &g, const P &p);
  T get(const P &p) { return p.t_; }
};
template <class T>
template <class G>
T D<T>::operator()(G &g, const P &p) { return p.t_ + g; }

template <class T> struct A {
  class P { T t_ = 6; friend class A<T>; };
  T get(const P &p) const { return p.t_; }
};
template <class T> struct C {
  class P { T t_ = 6; friend C; };
  T get(const P &p) const { return p.t_; }
};
template <class T> struct S;
template <class T> struct S<T *> {
  class P { T t_ = 9; friend class S; };
  T get(const P &p) const;
};
template <class T> T S<T *>::get(const P &p) const { return p.t_; }
// The elaborated-type-specifier outside a friend declaration is the
// injected-class-name too: `class B *` is `B<T> *`.
template <class T> struct B { class B *self; T v; };

int main() {
  D<int> d;
  D<int>::P p(3);
  int g = 4;
  B<int> b;
  b.self = &b;
  b.v = 5;
  printf("%d %d %d %d %d %d\n", d(g, p), d.get(p) + 4, A<int>().get({}),
         C<long>().get({}) > 5 ? 6 : 0, S<int *>().get({}), b.self->v);
}

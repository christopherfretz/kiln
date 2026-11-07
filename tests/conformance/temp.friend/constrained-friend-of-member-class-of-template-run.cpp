// [temp.friend]/9: a friend function template whose constraint names the
// enclosing class template's parameters declares a template of its own
// in each specialization. The injected-class-name of a member class of a
// class template, defined out of line or in the class, names those
// parameters too, so two specializations' friends are not redefinitions
// (a pipeable adaptor's `operator|` friends; WP-789).
//@ expect: run
//@ std: c++20
//@ stdout: 1 2 3 4
extern "C" int printf(const char *, ...);

template <class A, class B> concept same = __is_same(A, B);

template <class C> struct outer {
  struct type;
};
template <class C> struct outer<C>::type {
  template <class S>
    requires same<S, type>
  friend int f(S) { return sizeof(C) == 1 ? 1 : 2; }
};

template <class C> struct in_class {
  struct type {
    template <class S>
      requires same<S, type>
    friend int g(S) { return sizeof(C) == 1 ? 3 : 4; }
  };
};

int main() {
  outer<char>::type a;
  outer<int>::type b;
  in_class<char>::type c;
  in_class<int>::type d;
  printf("%d %d %d %d\n", f(a), f(b), g(c), g(d));
}

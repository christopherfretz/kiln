// Friend templates and the friends of class templates for the
// declaration image (spec/modules.md §11.11 "Friends", WP-1307).
export module imgft;
export namespace ft {
// A class that is not a template, befriending templates: a class
// template introduced by the friend declaration, a hidden friend
// function template defined in the class, and a function template
// declared there and defined after it.
struct A {
  constexpr A(int x) { v = x; }
  template <class U> friend struct B;
  template <class U> friend constexpr int f(U u, A a) { return a.v + (int)sizeof(u); }
  template <class U> friend int g(U, const A &);

private:
  int v;
};
template <class U> int g(U u, const A &a) { return a.v * 2 + (int)sizeof(u); }
template <class U> struct B {
  static int get(const A &a) { return a.v + 100; }
};
// A class template whose friend template is the first declaration of a
// class template defined later (libc++'s `__wrap_iter` and
// `basic_string`), with hidden friends, a friend function template, a
// non-template friend and a befriended class.
template <class T> struct It {
  template <class C, class Tr> friend class Str;
  friend int peek(const It &i) { return *i.p; }
  friend bool operator==(const It &a, const It &b) { return a.p == b.p; }
  template <class U> friend int dist(const It &a, const It<U> &b) { return (int)(a.p - b.p); }
  friend struct Probe;
  It() = default;

private:
  explicit It(T *q) { p = q; }
  T *p = nullptr;
};
struct Probe {
  template <class T> static T *get(const It<T> &i) { return i.p; }
};
template <class C, class Tr = int> class Str {
public:
  Str(C *q) { b = q; }
  It<C> begin() const;
  int size() const;

private:
  C *b;
};
template <class C, class Tr> It<C> Str<C, Tr>::begin() const { return It<C>(b); }
template <class C, class Tr> int Str<C, Tr>::size() const { return (int)sizeof(Tr) + 1; }
// A friend class template redeclaring one declared before the class
// template: its default argument stays the earlier declaration's.
template <class C, class D = long> struct Early;
template <class T> struct Holder {
  template <class C, class D> friend struct Early;

private:
  static constexpr int secret = 7;
};
template <class C, class D> struct Early {
  static constexpr int get() { return Holder<C>::secret + (int)sizeof(D); }
};
} // namespace ft

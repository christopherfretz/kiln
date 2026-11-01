// [class.copy.assign]/2, /4, [class.dtor]/2: the implicit copy and move
// assignment operators and destructor are declared members, so a
// using-declaration ([namespace.udecl]/1), a qualified name
// ([class.qual]) and a member access ([class.mfct.non.static]) find them
// before anything else used them (libc++ archetypes:
// `using Base::operator=;` in a class deriving a class template
// specialization). Matches clang 19.
//@ expect: run
template <class T> struct Base {
  T v;
  constexpr Base(T x) : v(x) {}
};
template <class T> struct Derived : Base<T> {
  using Base<T>::Base;
  using Base<T>::operator=;
};
constexpr int use_using() {
  Derived<int> a(1), b(2);
  a = b;
  return a.v;
}
static_assert(use_using() == 2);

struct S {
  int v;
};
constexpr int qualified() {
  S a{1}, b{3};
  a.S::operator=(b);
  return a.v;
}
static_assert(qualified() == 3);

template <class T> struct W {
  T v;
};
constexpr int member_access() {
  W<int> a{1}, b{4};
  a.operator=(static_cast<W<int>&&>(b));
  return a.v;
}
static_assert(member_access() == 4);

struct Box {
  union {
    W<int> w;
  };
  Box() : w{6} {}
  ~Box() {}
};
int destroy() {
  Box b;
  int r = b.w.v;
  W<int>* p = &b.w;
  p->~W();
  return r;
}

struct U {
  int v;
  constexpr U& assign(const U& o) { return operator=(o); }
};
constexpr int unqualified() {
  U a{1}, b{5};
  return a.assign(b).v;
}
static_assert(unqualified() == 5);

int main() { return use_using() + qualified() + member_access() + unqualified() + destroy() == 20 ? 0 : 1; }

// [dcl.type.elab]/1-3, [class.friend]/3, [temp.dep.type]/6: an
// elaborated-type-specifier `class-key T::X` whose qualifier is dependent
// (a template parameter or a dependent typedef) names a dependent type,
// resolved at instantiation; as `friend class-key T::X;` it befriends that
// class of the instantiation (libstdc++ generator:684
// `friend struct _Erased_promise::_Subyield_state;`; WP-332).
// Verified against clang 19.
//@ expect: run
//@ strict-diags
//@ stdout: 42 7 3
extern "C" int printf(const char *, ...);

template <class T> struct P { struct S; };

template <class T> struct G {
  using E = P<T>;
  friend struct E::S;

private:
  static constexpr int secret = 42;
};

template <class T> struct P<T>::S {
  static int get() { return G<T>::secret; }
};

struct Q {
  struct S { static int get(); };
  enum E { e3 = 3 };
};

template <class T> struct H {
  friend struct T::S;
  struct T::S *p = nullptr;
  enum T::E e = T::e3;

private:
  static constexpr int v = 7;
};

int Q::S::get() { return H<Q>::v; }

int main() {
  H<Q> h;
  printf("%d %d %d\n", P<int>::S::get(), Q::S::get(), int(h.e));
  return h.p != nullptr;
}

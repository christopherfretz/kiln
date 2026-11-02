// [temp.mem]/2: a local class (or a class nested in one) shall not have
// member templates: member function, class, variable and alias templates
// are "templates cannot be declared inside of a local class" at the
// `template` keyword; a friend function template is clang's "templates
// can only be declared in namespace or class scope". A generic lambda in
// a local class's member function is not a member template, and is
// allowed.
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 20:5 error "templates cannot be declared inside of a local class"
//@ diag: 21:5 error "templates cannot be declared inside of a local class"
//@ diag: 22:5 error "templates cannot be declared inside of a local class"
//@ diag: 23:5 error "templates cannot be declared inside of a local class"
//@ diag: 24:5 error "templates can only be declared in namespace or class scope"
//@ diag: 32:7 error "templates cannot be declared inside of a local class"
template <class T> void ft(T) {}
void g() {
  struct L {
    template <class T> void mf(T) {}
    template <class T> struct MC {};
    template <class T> static constexpr T mv = T();
    template <class T> using MA = T *;
    template <class T> friend void ft(T);
    void ok() {
      auto lam = [](auto x) { return x; };
      (void)lam(1);
    }
  };
  struct M {
    struct N {
      template <int> void h() {}
    };
  };
  L().ok();
}

// [class.friend]/11: the friend class of a local class in a member
// function of a class template is the class of that block scope; a class
// of the same name defined in a nested block is another class and no
// friend (diagnosed once, in the template; the -run sibling shows the
// block's own class).
//@ skip(clang): clang searches past the block scope and does not befriend a class defined later in it
//@ oracle: gcc
//@ expect: compile-fail
//@ std: c++20
//@ strict-diags
//@ diag: 18:50 error "'i' is a private member of 'L'"
//@ diag: 17:37 note "implicitly declared private here"

template <class T> struct S {
  int m() {
    {
      class L { friend class F; int i = sizeof(T); };
      { class F { public: int g(L &l) { return l.i; } }; return F().g(*new L); }
    }
  }
};
int x = S<int>().m();

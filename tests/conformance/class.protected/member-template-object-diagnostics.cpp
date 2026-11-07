// [class.protected]/1: a protected non-static member function template is
// usable from a derived class only through an object of that class; a
// static one needs no object, but a friend of a class that derives from
// the base privately still cannot name it in the base.
//@ verify
class Base {
 protected:
  template <class H> int Add(int v) { return v + 1; } // expected-note {{can only access this member on an object of type 'D'}}
  template <class H> static int SAdd(int v) { return v + 1; } // expected-note {{declared protected here}}
  static int splain(int v) { return v; } // expected-note {{declared protected here}}
};
struct D : Base {
  int g(Base* b) { return b->Add<int>(1); } // expected-error {{'Add' is a protected member of 'Base'}}
  int h(Base* b) { return b->SAdd<int>(1) + b->splain(1); }
  int k(D* d) { return d->Add<int>(1) + d->Base::Add<int>(1); }
};
class F final : private Base { friend struct NT; };
struct NT {
  int go(F* f) {
    return f->Base::SAdd<int>(3) // expected-error {{'SAdd' is a protected member of 'Base'}}
         + f->Base::splain(1); // expected-error {{'splain' is a protected member of 'Base'}}
  }
};

// [temp.dep.general]/3, [temp.dep.type]/1, [temp.res.general]/4,
// [temp.mem]/2: valid uses of classes declared in a dependent context. A
// local class instantiates other templates, deduces a class template's
// arguments (CTAD), and names its own members without `typename`; a
// `typename`-qualified name reaches a local class's member type from outside
// it; `this->m`, `A::m` and `B::m` reach a dependent base's members; a
// non-dependent base (`Global`) is searched by unqualified lookup; and an
// unqualified name a dependent base also declares skips that base and
// finds the namespace-scope variable (`shadowed`: 40, not the base's 1). A
// local class of a member function template, and a member class of a class
// template used as a base, complete the set. Run through codegen. WP-1084.
//@ expect: run
//@ strict-diags
//@ stdout: 78 22 7
extern "C" int printf(const char*, ...);

struct Global { int g = 5; };
int shadowed = 40;

template <class T> struct Box { T v; };
template <class T> Box(T) -> Box<T>;
template <class T> struct Holder { T items[2]; int size() const { return 2; } };

template <class T> int local_patterns(T t) {
  struct A { int z = 3; int shadowed = 1; int get() const { return z; } };
  struct B : A, Global {
    // Qualified names find the dependent base's members; the
    // non-dependent base is searched; `shadowed` skips A and finds ::shadowed.
    int g2() const { return this->z + A::z + B::z + this->get() + g; }
    int unqualified() const { return shadowed; }
    int via_this() const { return this->shadowed; }
  };
  struct L {
    T val;
    struct N { int q = 4; };
    // Inside L, L is the current instantiation: no typename needed.
    int own() const { L::N n; N m; return n.q + m.q; }
    int lambda() const { auto f = [](auto x) { return x + 1; }; return f(val); }
  };
  B b;
  Holder<B> h{};
  Box bx{L{t}};
  typename L::N n;
  int r = b.g2() + b.unqualified() + b.via_this() + h.size() + n.q
        + (int)sizeof(typename L::N) + bx.v.own() + bx.v.lambda();
  return r;
}

template <class T> struct X {
  struct M { int z = 6; };
  struct D : M { int g() { return this->z + M::z; } };
  template <class U> int m(U u) {
    struct Local { U u; int twice() const { return u + u; } };
    return Local{u}.twice() + D().g();
  }
};

template <class T> int nested(T) {
  struct Outer {
    struct Inner { struct N { int v = 7; }; };
    int m() const { typename Inner::N n; return n.v; }
  };
  return Outer().m();
}

int main() {
  printf("%d %d %d\n", local_patterns(1), X<int>().m(5), nested(0));
  return 0;
}

// [temp.res.general]/6, [over.ref]/1, [over.match.oper]: a resolved `->`
// chain in a template (an `operator->` returning a class object, by value
// or by reference, whose own `operator->` continues the chain) is formed
// again at instantiation, `const` and non-`const` `operator->` overloads
// chosen anew for the substituted object; a dependent `T *` operand keeps
// the built-in `->`. Evaluated in constant expressions.
//@ expect: compile-ok
//@ strict-diags

struct Obj {
  int x;
  constexpr int get() const { return x; }
  constexpr int get() { return x + 100; }
};

struct Inner {
  Obj *o;
  constexpr Obj *operator->() { return o; }
  constexpr const Obj *operator->() const { return o; }
};

// By value: the chain goes through a temporary `Inner`.
struct Outer {
  Obj *o;
  constexpr Inner operator->() const { return Inner{o}; }
};

// By reference: the chain goes through an lvalue `Inner`.
struct OuterRef {
  Inner in;
  constexpr Inner &operator->() { return in; }
  constexpr const Inner &operator->() const { return in; }
};

// Destructor in the chain: the temporary is bound for destruction.
struct Guard {
  Obj *o;
  int *dtors;
  constexpr Obj *operator->() const { return o; }
  constexpr ~Guard() { ++*dtors; }
};
struct Locker {
  Obj *o;
  int *dtors;
  constexpr Guard operator->() const { return Guard{o, dtors}; }
};

template <typename T> constexpr int chain(T t) {
  Obj o{t};
  Outer a{&o};
  int r = a->x + a->get(); // Inner temporary is non-const: get()
  OuterRef b{Inner{&o}};
  const OuterRef cb{Inner{&o}};
  r += b->get() + cb->get(); // non-const and const chains
  b->x = 2;
  r += o.x;
  int dtors = 0;
  Locker l{&o, &dtors};
  r += l->x + l->get();
  return r * 1000 + dtors;
}
// a: 1 + 101; b: 101 + cb: 1; o.x = 2; l: 2 + 102; two Guard destructions.
static_assert(chain(1) == 310002);

// A dependent `T *`: the built-in `->`, substituted unchanged.
template <typename T> constexpr int builtin(T *p) { return p->x + p->get(); }
constexpr int use_builtin() {
  Obj o{5};
  const Obj *c = &o;
  return builtin(&o) * 1000 + builtin(c);
}
static_assert(use_builtin() == 110010);

// A member template's `->` chain with an operand of the enclosing class
// template's type, non-dependent in the member.
template <typename U> struct Holder {
  Obj o{7};
  template <typename V> constexpr int via() const {
    Inner in{const_cast<Obj *>(&o)};
    const Inner &c = in;
    return in->get() + c->get();
  }
};
static_assert(Holder<int>{}.via<char>() == 114);

// [expr.prim.id.dtor], [expr.ref]/4: a pseudo-destructor call through
// `operator->` (one link and a chain): each `operator->` call is selected
// again for the substituted object.
struct IntBox {
  int *p;
  int *operator->() const { return p; }
};
struct IntBoxes {
  int *p;
  IntBox operator->() const { return IntBox{p}; }
};
using I = int;
template <typename T> void pseudo(T t) {
  int x = t, y = t;
  IntBox b{&x};
  b->~I();
  IntBoxes c{&y};
  c->~I();
}
void use_pseudo() { pseudo(0); }

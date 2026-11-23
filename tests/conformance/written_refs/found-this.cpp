// Found sets of members of the current instantiation (spec/ast.md
// §18.12, [temp.dep.type]/6): `this->a`, `C::a` and `a` with an implicit
// `this` are looked up in the class template at the definition
// ([class.member.lookup], [class.mem.general]: a member function body is
// a complete-class context, so `late` declared after `use` is found);
// the class has no dependent base, so each set is closed (`def=n`). A
// member template named with `template` and explicit arguments is looked
// up too. The rows select the specialization's members, instantiated
// from the elements.
//@ written-refs: found-this.refs
template <class T> struct C {
  void a(T);
  void a(T, int);
  static void st(T);
  static void st(T, int);
  template <class U> void mt(U);
  void use(T t) {
    this->a(t);
    C::a(t);
    a(t);
    st(t);
    this->mt(t);
    this->template mt<T>(t);
    late(t);
  }
  void late(T);
  void late(T, int);
};
void run() { C<int>{}.use(1); }

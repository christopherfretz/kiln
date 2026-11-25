// Found sets in a class template with a dependent base (spec/ast.md
// §18.12): a member of the current instantiation is looked up in the
// class at the definition; the class declares `a` itself, which hides
// any base's ([class.member.lookup]/2), so the set is closed (no
// `+base`, [temp.dep.type]/6). A name the class does not declare
// (`this->other`) is a member of an unknown specialization and a
// qualified name with a dependent qualifier (`B<T>::other`) names a
// member of one too: no lookup runs at the definition and there is no
// set. A name a dependent using-declaration introduces is an element of
// its own, marked `+using` ([namespace.udecl]/20).
//@ written-refs: found-this-base.refs
template <class T> struct B {
  void bm(T);
  void bm(T, int);
  void other(T);
};
template <class T> struct D : B<T> {
  void a(T);
  void a(T, int);
  void use(T t) {
    this->a(t);
    D::a(t);
    a(t);
    this->other(t);
    B<T>::other(t);
  }
  using B<T>::bm;
  void use2(T t) { bm(t); }
};
void run() {
  D<int> d;
  d.use(1);
  d.use2(1);
}

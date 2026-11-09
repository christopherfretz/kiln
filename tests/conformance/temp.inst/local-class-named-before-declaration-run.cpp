// [basic.scope.pdecl]/7, [temp.inst]/3: `struct L *p` in a function
// template body declares the local class L; the instantiation declares it
// when the pointer's type is substituted, ahead of L's declaration
// statement, which then defines the same class (clang `FindInstantiatedDecl`
// `NeedInstantiate`). kiln reported "not yet implemented: reference to a
// member of a dependent context". WP-1103.
//@ expect: run

template <class T> T *f() {
  struct L *p = nullptr;
  struct L { T v; };
  static L l{T(3)};
  p = &l;
  return &p->v;
}
int main() { return *f<int>() == 3 && *f<long>() == 3 ? 0 : 1; }

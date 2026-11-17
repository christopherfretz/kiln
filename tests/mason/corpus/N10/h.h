// implicitly defined special members (the destructor is virtual: the vtable keeps it)
struct M {
  M();
  ~M();
  int v;
};
struct B {
  virtual ~B();
};
struct X : B {
  M m;
  int k = 2;
};
inline X *mk() { return new X; }

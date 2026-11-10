// The interface of module_vtable_o1_importer.cpp: a purview class whose
// key function (its destructor) this unit defines.
export module mod_vtable_o1;
export struct A {
  virtual ~A();
  virtual int f() const { return 4; }
};
A::~A() {}

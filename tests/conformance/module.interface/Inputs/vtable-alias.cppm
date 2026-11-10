// The module unit for vtable-alias-O1-run.cpp: a class whose key
// function (its destructor) is defined in this unit.
export module vta;
export struct A {
  virtual ~A();
  virtual int f() const { return 4; }
};
A::~A() {}

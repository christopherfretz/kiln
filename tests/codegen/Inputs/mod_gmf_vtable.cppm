// The interface of module_gmf_vtable.cpp: a global-module-fragment class
// with a key function the fragment defines (G), one whose virtual
// functions are all inline (GI), and a purview class with a key function
// (M).
module;
struct G {
  virtual int f();
  virtual ~G() {}
};
int G::f() { return 1; }
struct GI {
  virtual int f() { return 2; }
};
export module mod_gmf_vtable;
export using ::G;
export using ::GI;
export struct M {
  virtual int f();
};
int M::f() { return 3; }

// The interface unit of gmf-dup-run.cpp: global-module-fragment
// definitions (a non-inline function, a variable, a class with a key
// function, a class whose virtual functions are all inline) and purview
// classes whose vtables it owns.
module;
int gmf_counter = 40;
int gmf_next() { return ++gmf_counter; }
struct G {
  virtual int f();
  virtual ~G() {}
};
int G::f() { return 1; }
struct GI {
  virtual int f() { return 2; }
};
export module gd;
export using ::G;
export using ::gmf_next;
// [class.mfct]/1: not inline (attached to a named module), so defined
// only here.
export struct ML {
  virtual int f() { return 3; }
  int g() { return 4; }
};
export struct MK {
  virtual int f();
  virtual ~MK() = default;
};
int MK::f() { return 5; }
export inline int use_gi() {
  GI gi;
  return gi.f();
}

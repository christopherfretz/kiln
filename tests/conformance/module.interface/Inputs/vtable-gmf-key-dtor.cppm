// The module unit for vtable-gmf-key-dtor-O1-run.cpp: a global-module-
// fragment class whose key function (its destructor) the fragment
// defines.
module;
struct G {
  virtual ~G();
  virtual int f() const;
};
G::~G() {}
int G::f() const { return 5; }
export module vtg;
export using ::G;

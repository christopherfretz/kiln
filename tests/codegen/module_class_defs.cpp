// [class.mfct]/1, [class.friend]/7: a function defined in the definition
// of a class attached to a named module is not implicitly inline, so the
// interface unit emits it strongly, used or not, right after the
// namespace-scope class that contains it. A dynamic class defined in the
// purview has its vtable emitted by the interface unit even when unused
// (clang `MarkVTableUsed` with `DefinitionRequired`), with its defaulted
// virtual destructor linkonce_odr; the implicit global operator delete it
// calls is attached to the global module ([basic.stc.dynamic.general]/2).
// (clang 19 gives _ZGIW no function attributes; kiln follows clang 22.)
//@ min-oracle: 22
export module mcd;
int top() { return 1; }
struct P {
  int p() { return 1; }
  int q();
  struct In {
    int in() { return 2; }
  };
};
export struct Q {
  int q() { return 3; }
  friend int fr(const Q &) { return 4; }
};
int P::q() { return 5; }
struct V {
  virtual int v() { return 6; }
};
namespace ns {
struct N {
  int n() { return 7; }
};
}
export struct MT {
  virtual ~MT() = default;
};

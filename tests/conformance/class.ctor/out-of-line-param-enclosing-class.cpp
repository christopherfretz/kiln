// [basic.scope.class]/1, [basic.lookup.unqual]/3, [class.access.general]/7:
// after the qualified declarator-id of an out-of-line constructor, the
// parameter-declaration-clause is in the scope of the constructor's class,
// so `Inner` is found through the enclosing classes of `Ref`, and access
// to the protected `Inner` is checked in the member's context. A
// regression in 1.2.0 (WP-1455), reduced from LLVM's DWARFLinker.
//@ expect: compile-ok

class Outer {
protected:
  struct Inner {
    struct Ref {
      Ref(int);
      Ref(Inner::Ref &&Other);
      int V;
    };
  };
};
Outer::Inner::Ref::Ref(int v) : V(v) {}
Outer::Inner::Ref::Ref(Inner::Ref &&Other) : V(Other.V) {}

// Translation unit #1 of the [basic.lookup.argdep]/5 example.
export module M;
namespace R {
  export struct X {};
  export void f(X);
}
namespace S {
  export void f(R::X, R::X);
}

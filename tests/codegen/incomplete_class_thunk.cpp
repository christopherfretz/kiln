// WP-462: the thunk of a virtual function whose class parameter or return
// type is never completed is declared `void ()` without attributes, as
// clang `GetAddrOfThunk` declares a function type `isFuncTypeConvertible`
// rejects. A variadic covariant thunk is a plain declaration at -O0.
struct I;
struct A { virtual int f(int); int a; };
struct B { virtual int g(I); virtual I &r(); virtual B *h(int, ...); int b; };
struct C : A, B { virtual void k(); int g(I) override; I &r() override; C *h(int, ...) override; };
void C::k() {}

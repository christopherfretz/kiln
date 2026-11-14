// WP-1411: `-fstack-clash-protection` gives "probe-stack"="inline-asm"
// and `-mstack-probe-size=8192` "stack-probe-size"="8192" on every
// function kiln defines, its synthesized ones included (the global
// initializer, the thunk), as clang 19's `StartFunction` and
// `addStackProbeTargetAttributes`; declarations get neither
// (spec/driver.md "Codegen flags"). The module also gets the
// "probe-stack" and "stack-probe-size" flags, as clang's `Release`.
//@ args: -fstack-clash-protection -mstack-probe-size=8192
int f();
int g = f();
struct A { virtual ~A(); };
struct B { virtual void x(); };
struct C : A, B { void x() override; ~C(); };
void C::x() {}
C::~C() {}
int big(int n) { char buf[100000]; buf[n] = 1; return buf[n / 2]; }

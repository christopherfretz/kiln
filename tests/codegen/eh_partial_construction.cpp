// [except.ctor]/3-4: partially constructed objects (bases, virtual
// bases, members, member arrays, delegating constructors, defaulted
// copy constructors) are destroyed when construction throws.
void f();
int g(int);
struct S { int a; S(); S(int); S(const S&); ~S(); };
struct T { int b; T(const T&) = default; ~T(); };
struct VB { VB(); ~VB(); };
struct A : virtual VB { S s; A() try : s(1) { f(); } catch (...) {} ~A() try { f(); } catch (...) {} };
struct B : A { B() : A() { f(); } };
void useb() { B b; A a; }

struct D {
  S arr[3];
  S s;
  D() : s(2) { f(); }
  D(int) : D() { f(); }
};
void used() { D d(1); }

struct E { T t1; T t2; int k; S s; E(const E&) = default; };
void copye(const E& e) { E e2(e); }

union UU { S s; int i; UU() {} ~UU() {} };
struct U { UU u; S s2; U() : u(), s2() { f(); } ~U() {} };
void useu() { U u; }

struct F { S s; F() try : s() {} catch (...) { throw; } };
void usef() { F x; }


// [except.throw]/2, [except.handle]/3 under -fno-rtti: the clang 19 IR
// (sibling of ir-fno-rtti-type-info.cpp, which says what is pinned; clang
// 19 creates a `type_info` global after its name and bases). WP-720.
//@ requires: clang<22
//@ expect: ir
//@ args: -fno-rtti -ftrivial-auto-var-init=uninitialized
struct Extern {           // key function defined in another TU
  virtual ~Extern();
  virtual int f() const;
};
struct Inline : Extern {  // no key function: linkonce_odr vtable
  int f() const override { return 2; }
};
struct Keyed : Extern {   // key function defined here: strong vtable
  int f() const override;
};
int Keyed::f() const { return 3; }

[[noreturn]] void thrower() { throw Inline(); }

int catcher(int x) {
  int r = 0;
  try {
    if (x) thrower();
  } catch (const Extern& e) {
    r = e.f();
  } catch (int*) {
    r = 1;
  }
  return r;
}

int use(const Extern& e) { return e.f(); }
int make() { Inline i; Keyed k; return use(i) + use(k); }

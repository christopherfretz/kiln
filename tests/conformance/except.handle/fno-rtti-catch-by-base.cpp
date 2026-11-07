// [except.handle]/3 under -fno-rtti: a handler for a base class (by
// reference and by pointer) still catches a derived exception object,
// whose `type_info` objects (and their bases') the throwing TU defines as
// `linkonce_odr` although the vtables' RTTI slots are null
// (spec/codegen.md "RTTI", under -fno-rtti). WP-720.
//@ expect: run
//@ args: -fno-rtti
//@ stdout: 7 3 1
#include <cstdio>
struct Base { virtual ~Base(); virtual int f() const { return 1; } };
Base::~Base() {}
struct Mid : Base { int f() const override { return 3; } };
struct Leaf : Mid { int f() const override { return 7; } };
static Leaf leaf;
int by_ref() { try { throw Leaf(); } catch (const Base& b) { return b.f(); } return 0; }
int by_ptr() { try { throw static_cast<Mid*>(&leaf); } catch (Mid* m) { return m->Mid::f(); } return 0; }
int other() { try { throw 1; } catch (const Base&) { return 0; } catch (int) { return 1; } return 0; }
int main() { std::printf("%d %d %d\n", by_ref(), by_ptr(), other()); }

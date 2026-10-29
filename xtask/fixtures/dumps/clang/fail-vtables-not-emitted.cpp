//@ expect: vtables
// No key function is defined, so clang emits (and dumps) no vtable.
struct P { virtual void f(); };

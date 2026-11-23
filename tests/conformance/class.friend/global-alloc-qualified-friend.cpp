// [class.friend]/11, [dcl.meaning.general]/2, [basic.stc.dynamic.general]/2
// (WP-2540): a `::`-qualified friend naming a global allocation function
// that only the implicit declarations declare redeclares the implicit one
// (it was "does not match any declaration in the global namespace"); an
// unqualified one redeclares it too, and a later declaration redeclares
// that friend.
//@ expect: compile-ok
//@ strict-diags
struct B { friend void *operator new(decltype(sizeof 0)); };
struct C {
  friend void *::operator new[](decltype(sizeof 0));
  friend void ::operator delete[](void *) noexcept;
};
void *operator new(decltype(sizeof 0));
int *f() { return new int[2]; }

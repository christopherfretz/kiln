// [class.copy.assign]/2,4, [class.friend]/4: a qualified friend
// declaration names the implicitly declared copy and move assignment
// operators of a complete class, here implicitly deleted (a const member,
// a member whose copy assignment is implicitly deleted). WP-958 sibling of
// implicit-form-const-ref.cpp.
//@ std: c++11
//@ expect: compile-ok
struct M {
  M &operator=(M &&);
};
struct V {
  M m;
};
struct W {
  const int a = 0;
};
struct G {
  friend V &V::operator=(V &&);
  friend W &W::operator=(W &&) noexcept;
  friend W &W::operator=(const W &) noexcept;
};

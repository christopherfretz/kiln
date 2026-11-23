// The found set (spec/ast.md §18.12): a hidden friend is invisible to
// ordinary lookup ([class.friend]/7, [namespace.memdef]), so it is never
// an element; argument-dependent lookup at instantiation finds it as a
// member of an associated class ([basic.lookup.argdep]/4.2), and it
// appears only on the row (`how=adl`).
//@ written-refs: found-hidden-friend.refs
namespace lib {
struct H {
  friend void hf(H) {}
};
} // namespace lib
template <class T> void call(T t) { hf(t); }
void use() { call(lib::H{}); }

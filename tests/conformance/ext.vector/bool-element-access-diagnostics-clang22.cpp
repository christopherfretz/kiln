// Clang ext_vector_type element access ([clang] "Vectors and Extended
// Vectors"; clang CheckExtVectorComponent): a `bool` ext vector has no
// element access (`v.x`, `p->yz`, `v.s01`, `v.hi`), since a lane is one
// bit and has no address; every accessor, valid or not, is "illegal
// vector component name" at the name, also after template substitution.
// A subscript is the way to read a lane. WP-438.
//@ expect: compile-fail
//@ args: -ferror-limit=0
//@ strict-diags
//@ diag: 21:38 error "illegal vector component name 'x'"
//@ diag: 21:46 error "illegal vector component name 'yz'"
//@ diag: 21:56 error "illegal vector component name 's01'"
//@ diag: 21:65 error "illegal vector component name 'hi'"
//@ diag: 21:75 error "illegal vector component name 'q'"
//@ diag: 22:40 error "illegal vector component name 'w'"
//@ diag: 23:46 error "illegal vector component name 'x'"
//@ diag: 24:10 note "in instantiation of function template specialization 'lane<bool __attribute__((ext_vector_type(4)))>' requested here"
//@ requires: clang>=22
// clang 22 spelling; bool-element-access-diagnostics.cpp has clang 19's.
typedef bool b4 __attribute__((ext_vector_type(4)));
b4 g; bool f(b4 v, b4 *p) { return v.x || p->yz.x || v.s01 || g.hi.x || v.q; }
constexpr bool c = ((b4){true, false}).w;
template <class V> bool lane(V v) { return v.x; }
bool h = lane(g);
bool ok(b4 v, b4 *p) { return v[0] || (*p)[3]; }

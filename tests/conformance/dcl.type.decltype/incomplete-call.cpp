// [dcl.type.decltype]/2: a prvalue function call operand of decltype need
// not have a complete type and no temporary is materialized.
//@ strict-diags
struct In;
In make();
In &make_ref();
static_assert(__is_same(decltype(make()), In));
static_assert(__is_same(decltype(make_ref()), In &));
template <class T> struct Box { T t; };
Box<In> box();
static_assert(__is_same(decltype(box()), Box<In>));

// [temp.inst]/2, [class.member.lookup]: a qualified name into a
// specialization looks in the instantiated class.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 7:11 error "no type named 'value_type' in 'Box<int>'"
template <class T> struct Box { typedef T type; };
Box<int>::value_type v;

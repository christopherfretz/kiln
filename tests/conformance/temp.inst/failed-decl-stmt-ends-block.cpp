// [temp.inst]/11, [dcl.pre]/10: instantiating a function body, a declaration
// statement that does not instantiate at all (a static_assert whose condition
// is invalid, a variable whose type does not substitute) ends its compound
// statement: the statements after it are not instantiated and report
// nothing (clang `TransformCompoundStmt`). A false static_assert, a variable
// with an invalid initializer and an invalid alias do not. libc++'s
// allocate.verify expects one error per allocator<incomplete>::allocate.
// spec/sema-templates.md "Function bodies".
//@ strict-diags
//@ expect: compile-fail
//@ diag: 36:44 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 42:14 note "in instantiation of function template specialization 'a<inc>' requested here"
//@ diag: 35:8 note "forward declaration of 'inc'"
//@ diag: 36:44 error "invalid application of 'sizeof' to an incomplete type 'void'"
//@ diag: 42:24 note "in instantiation of function template specialization 'a<void>' requested here"
//@ diag: 37:39 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 42:35 note "in instantiation of function template specialization 'b<int>' requested here"
//@ diag: 38:46 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 42:45 note "in instantiation of function template specialization 'c<inc>' requested here"
//@ diag: 35:8 note "forward declaration of 'inc'"
//@ diag: 38:71 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 35:8 note "forward declaration of 'inc'"
//@ diag: 39:38 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 42:55 note "in instantiation of function template specialization 'd<inc>' requested here"
//@ diag: 35:8 note "forward declaration of 'inc'"
//@ diag: 39:56 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 35:8 note "forward declaration of 'inc'"
//@ diag: 40:49 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 42:65 note "in instantiation of function template specialization 'e<int>' requested here"
//@ diag: 40:65 error "type 'int' cannot be used prior to '::' because it has no members"
//@ diag: 41:44 error "static assertion failed due to requirement '__is_same(inc, int)'"
//@ diag: 42:75 note "in instantiation of function template specialization 'f<inc>' requested here"
//@ diag: 41:71 error "invalid application of 'sizeof' to an incomplete type 'inc'"
//@ diag: 35:8 note "forward declaration of 'inc'"
struct inc;
template <class T> int a() { static_assert(sizeof(T) >= 0); return sizeof(T); }
template <class T> int b() { typename T::type x; return T::v; }
template <class T> int c() { { static_assert(sizeof(T) > 0); } return sizeof(T); }
template <class T> int d() { int x = sizeof(T); return sizeof(T); }
template <class T> int e() { using U = typename T::type; return T::v; }
template <class T> int f() { static_assert(__is_same(T, int)); return sizeof(T); }
void use() { a<inc>(); a<void>(); b<int>(); c<inc>(); d<inc>(); e<int>(); f<inc>(); }

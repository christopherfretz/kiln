// [dcl.init.list]/3.8: `E{v}` rejects a narrowing conversion of `v` to
// the underlying type, as `E e{v};` does.
//@ expect: compile-fail
//@ diag: 6:9 error "constant expression evaluates to 300 which cannot be narrowed to type 'E'"
enum class E : unsigned char {};
E a = E{300};
E b = E{30};

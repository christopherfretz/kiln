// [expr.reinterpret.cast]/4: std::nullptr_t converts to an integral type
// with the validity of `(void*)0`'s conversion, so not to a type smaller than
// a pointer; and not to an enumeration. WP-296.
//@ expect: compile-fail
//@ strict-diags
//@ diag: 8:9 error "cast from pointer to smaller type 'int' loses information"
//@ diag: 10:7 error "reinterpret_cast from 'std::nullptr_t' to 'E' is not allowed"
int i = reinterpret_cast<int>(nullptr);
enum E : long { e };
E j = reinterpret_cast<E>(nullptr);

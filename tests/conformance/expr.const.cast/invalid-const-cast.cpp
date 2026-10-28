// [expr.const.cast]/1: const_cast only converts pointers and references
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:9 error "const_cast to 'int', which is not a reference, pointer-to-object, or pointer-to-data-member"
int i = const_cast<int>(1.0);

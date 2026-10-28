// [expr.reinterpret.cast]/2: no reinterpret_cast between arithmetic types
//@ expect: compile-fail
//@ strict-diags
//@ diag: 5:11 error "reinterpret_cast from 'int' to 'float' is not allowed"
float f = reinterpret_cast<float>(1);

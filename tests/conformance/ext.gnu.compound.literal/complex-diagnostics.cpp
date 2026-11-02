// A complex compound literal takes at most two initializers, the real and
// imaginary parts (GNU ext; clang `CheckComplexType`): a third is excess.
// It is not "function-style cast ... can only take one argument".
//@ expect: compile-fail
//@ strict-diags
//@ diag: 10:61 error "excess elements in scalar initializer"
//@ diag: 11:56 error "excess elements in scalar initializer"
_Complex float ok(float x) { return (_Complex float){x, 2}; }
_Complex int ok2() { return (_Complex int){}; }
_Complex float bad(float x) { return (_Complex float){x, 2, 3}; }
_Complex int bad2(int x) { return (_Complex int){x, x, x, x}; }

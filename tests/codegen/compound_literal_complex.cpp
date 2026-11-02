// A complex compound literal with two initializers sets the real and
// imaginary parts (GNU ext; clang `InitListChecker::CheckComplexType`),
// one initializer the real part only; a constant one folds.
_Complex float b(float x) { return (_Complex float){x, 2}; }
_Complex float b2(float x) { _Complex float c = {x, 2}; return c; }
_Complex double b3(float x) { return (_Complex double){x}; }
_Complex int b4(int x, int y) { return (_Complex int){x + y, x - y}; }
float b5(float x) { return __imag__ (_Complex float){x, x + 1}; }
_Complex float g = (_Complex float){1, 2};

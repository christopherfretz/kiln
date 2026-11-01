// GNU imaginary constants (clang `VisitImaginaryLiteral`: a null real
// part beside the scalar imaginary part), as constant global initializers
// and inside complex arithmetic (WP-211).
_Complex double c = 2.0i;
_Complex int d = 3i;
_Complex double e = 1.5fi + 2.0j;
_Complex long f = 1il;
_Complex unsigned g = 7ui;
_Complex double h() { return 4.0i * c; }
_Complex float k(float x) { return x + 1.0fi; }
_Complex int m(int n) { return n * 2i - 1; }

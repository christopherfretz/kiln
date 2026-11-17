enum E { A, X, B };
inline int isB(int e) { return e == B; }
int (*keep_b)(int) = isB;

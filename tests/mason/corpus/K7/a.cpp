enum E { A, B };
inline int isB(int e) { return e == B; }
int (*keep_a)(int) = isB;

enum E { A, B = 2 };
inline int isB(int e) { return e == B; }
int (*keep_a)(int) = isB;

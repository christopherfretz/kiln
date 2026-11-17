enum E { A, B = 3 };
inline int isB(int e) { return e == B; }
int (*keep_b)(int) = isB;

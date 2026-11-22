#define SHARED(x) (x + 1)
inline int shared_value = SHARED(SHARED(2));

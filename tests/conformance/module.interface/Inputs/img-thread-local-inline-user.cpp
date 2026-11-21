// The second importer of image-thread-local-inline-run.cpp.
import imgtli;
int user_sum() { return inl + st + S::si + Box<int>::v + vt<int> + bump<int>() + counter(); }
int *user_inl() { return &inl; }

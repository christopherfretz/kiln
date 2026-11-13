int main_uses_internal() { return internal_undefined(); }
int main_calls_old() { return pfx_calls_old(1) + pfx_calls_old('c'); }
int main_null() { return pfx_null(1) + pfx_null(2L); }

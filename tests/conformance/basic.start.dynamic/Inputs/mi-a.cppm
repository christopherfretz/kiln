// Module a of module-init-run.cpp: one dynamically initialized variable.
export module a;
import seq;
export int order_a = seq_take();

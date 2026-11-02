// Module b of module-init-run.cpp: imports a, re-exports c; its variable
// is initialized after both of theirs ([basic.start.dynamic]/5).
export module b;
export import c;
import a;
import seq;
export int order_b = seq_take() + order_a * 0;

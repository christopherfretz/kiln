//! The hook seams of [`Preprocessor`]: custom [`IncludeResolver`],
//! [`IfEvaluator`], [`FeatureQueries`] and [`PpCallbacks`] plug in without
//! core changes (WP-014 .. WP-017), and the [`Marker`]s of
//! [`Preprocessor::next_item`].
//!
//! Hand-written: these check the interface, not clang parity.

mod common;

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;

use kiln_diag::{DiagEngine, DiagOptions, MemorySink};
use kiln_lex::{Interner, Std, Symbol, TokKind};
use kiln_pp::{
    FeatureQueries, FileChange, IfContext, IfEvaluator, IncludeKind, IncludeRequest,
    IncludeResolver, MacroDef, Marker, MemoryResolver, PpCallbacks, PpItem, PpOptions, PragmaInfo,
    PragmaMessageKind, Preprocessor, PrintOptions, ResolvedInclude, TargetQuery,
    print_preprocessed,
};
use kiln_source::{SourceManager, Span};

type Log = Rc<RefCell<Vec<String>>>;

/// Runs `src` with hooks installed by `setup`; returns the `-E -P` text
/// and the diagnostics' messages.
fn run(
    src: &str,
    setup: impl for<'a, 'd> FnOnce(&mut Preprocessor<'a, 'd>, &'a Interner),
) -> (String, Vec<String>) {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let sink = MemorySink::new();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), sink.clone());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    setup(&mut pp, &interner);
    pp.enter_main_file(main);
    let mut out = String::new();
    print_preprocessed(&mut pp, PrintOptions::default(), &mut out);
    drop(pp);
    (out, sink.take().iter().map(|d| d.message.clone()).collect())
}

/// Every name resolves to a buffer `name_<name>`; requests are logged.
struct Synth(Log);

impl IncludeResolver for Synth {
    fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
        self.0.borrow_mut().push(format!(
            "{:?} {} angled={} from={:?}",
            req.kind, req.name, req.angled, req.lookup_from
        ));
        if req.name == "missing.h" {
            return None;
        }
        let file =
            sm.add_virtual(&req.name, format!("name_{}\n", req.name.replace('.', "_"))).ok()?;
        Some(ResolvedInclude { file, lookup_index: Some(3), system: false })
    }
}

/// [cpp.include]/2: the resolver decides what a header name means.
#[test]
fn custom_include_resolver() {
    let log = Log::default();
    let (out, diags) = run(
        "#include \"x.h\"\n#include <y.h>\n#if __has_include(<missing.h>)\nbad\n#endif\n",
        |pp, _| pp.set_include_resolver(Box::new(Synth(log.clone()))),
    );
    assert_eq!(out, "name_x_h\nname_y_h\n\n");
    assert!(diags.is_empty(), "{diags:?}");
    assert_eq!(
        *log.borrow(),
        [
            "Include x.h angled=false from=None",
            "Include y.h angled=true from=None",
            "HasInclude missing.h angled=true from=None",
        ]
    );
}

/// `#include_next` passes the includer's lookup index on.
#[test]
fn include_next_lookup_from() {
    struct Next(Log);
    impl IncludeResolver for Next {
        fn resolve(&mut self, sm: &SourceManager, req: &IncludeRequest) -> Option<ResolvedInclude> {
            self.0.borrow_mut().push(format!("{:?} {:?}", req.kind, req.lookup_from));
            let (text, idx) = match req.kind {
                IncludeKind::Include => ("#include_next <a.h>\n", 2),
                _ => ("second\n", 3),
            };
            let file = sm.add_virtual(&format!("{idx}/a.h"), text.to_owned()).ok()?;
            Some(ResolvedInclude { file, lookup_index: Some(idx), system: false })
        }
    }
    let log = Log::default();
    let (out, _) =
        run("#include <a.h>\n", |pp, _| pp.set_include_resolver(Box::new(Next(log.clone()))));
    assert_eq!(out, "second\n\n");
    assert_eq!(*log.borrow(), ["Include None", "IncludeNext Some(2)"]);
}

/// [cpp.cond]: the evaluator owns the expression grammar (WP-015).
#[test]
fn custom_if_evaluator() {
    /// True iff the expression's first token is the identifier `yes`.
    struct Yes;
    impl IfEvaluator for Yes {
        fn eval(&mut self, cx: &mut dyn IfContext) -> bool {
            let first = cx.next_unexpanded();
            let yes = cx.spelling(&first) == "yes";
            while cx.next().kind != TokKind::Eod {}
            yes
        }
    }
    let (out, diags) = run(
        "#if yes @ $ \"anything\"\na\n#elif yes\nb\n#endif\n#if no\nc\n#else\nd\n#endif\n",
        |pp, _| pp.set_if_evaluator(Box::new(Yes)),
    );
    assert_eq!(out, "\na\nd\n");
    assert!(diags.is_empty(), "{diags:?}");
}

/// The answers of WP-017's feature tables.
struct Features;

impl FeatureQueries for Features {
    fn has_feature(&self, name: &str) -> bool {
        name == "kiln_ovens"
    }
    fn has_builtin(&self, name: &str) -> u32 {
        u32::from(name == "__builtin_bake") * 7
    }
    fn has_attribute(&self, name: &str) -> u32 {
        u32::from(name == "glaze")
    }
    fn has_cpp_attribute(&self, scope: Option<&str>, name: &str) -> u32 {
        if scope == Some("kiln") && name == "fired" { 2026 } else { 0 }
    }
    fn is_keyword(&self, name: &str) -> bool {
        name == "kilnword"
    }
    fn is_target(&self, query: TargetQuery, name: &str) -> bool {
        query == TargetQuery::Arch && name == "aarch64"
    }
}

/// [cpp.cond]/5-7: `__has_*` answers come from [`FeatureQueries`].
#[test]
fn custom_feature_queries() {
    let src = "__has_feature(kiln_ovens) __has_feature(other) __has_extension(kiln_ovens)\n\
               __has_builtin(__builtin_bake) __has_builtin(__builtin_other)\n\
               __has_attribute(glaze) __has_cpp_attribute(kiln::fired) __has_cpp_attribute(fired)\n\
               __is_identifier(kilnword) __is_identifier(int)\n\
               __is_target_arch(aarch64) __is_target_os(aarch64)\n";
    let (out, diags) = run(src, |pp, _| pp.set_feature_queries(Box::new(Features)));
    // Values other than 0 and 1 get an `L` suffix (as in clang).
    let lines: Vec<_> = out.lines().map(str::trim_start).collect();
    assert_eq!(lines, ["1 0 1", "7L 0", "1 2026L 0", "0 1", "1 0"]);
    assert!(diags.is_empty(), "{diags:?}");
}

/// Logs every callback.
struct Spy<'i> {
    log: Log,
    interner: &'i Interner,
}

impl Spy<'_> {
    fn push(&self, s: String) {
        self.log.borrow_mut().push(s);
    }
    fn name(&self, s: Symbol) -> &str {
        self.interner.as_str(s)
    }
}

impl PpCallbacks for Spy<'_> {
    fn file_changed(&mut self, change: FileChange) {
        let s = match change {
            FileChange::Enter { from, .. } => format!("enter from_include={}", from.is_some()),
            FileChange::Exit { .. } => "exit".to_owned(),
            FileChange::Rename { flag, .. } => format!("rename {flag:?}"),
            FileChange::SystemHeader { .. } => "system_header".to_owned(),
        };
        self.push(s);
    }
    fn inclusion_directive(&mut self, req: &IncludeRequest, result: Option<&ResolvedInclude>) {
        self.push(format!("include {} found={}", req.name, result.is_some()));
    }
    fn macro_defined(&mut self, name: Symbol, def: &MacroDef) {
        if !self.name(name).starts_with("__") {
            self.push(format!("define {} fn={}", self.name(name), def.is_function_like()));
        }
    }
    fn macro_undefined(&mut self, name: Symbol, _span: Span, def: Option<&MacroDef>) {
        self.push(format!("undef {} had={}", self.name(name), def.is_some()));
    }
    fn macro_expands(&mut self, name: &kiln_lex::Token, def: &MacroDef, _range: Span) {
        if let TokKind::Identifier(s) = name.kind {
            self.push(format!("expand {} body={}", self.name(s), def.body().len()));
        }
    }
    fn pragma(&mut self, info: &PragmaInfo<'_>) -> bool {
        let consume = matches!(info.tokens.first().map(|t| t.kind),
            Some(TokKind::Identifier(s)) if self.name(s) == "kiln");
        self.push(format!(
            "pragma {:?} n={} consumed={consume}",
            info.introducer,
            info.tokens.len()
        ));
        consume
    }
    fn pragma_message(
        &mut self,
        _span: Span,
        ns: Option<&str>,
        kind: PragmaMessageKind,
        text: &str,
    ) {
        self.push(format!("message {ns:?} {kind:?} {text}"));
    }
    fn ident(&mut self, _span: Span, text: &str) {
        self.push(format!("ident {text}"));
    }
    fn source_range_skipped(&mut self, _range: Span) {
        self.push("skipped".to_owned());
    }
}

/// Observers see directives, expansions and files (WP-017's
/// `PPCallbacks`), and pragma handlers can consume pragmas.
#[test]
fn custom_callbacks() {
    let log = Log::default();
    let src = "#define F(x) x x\n#define O 1\nF(O)\n#undef O\n#undef nope\n#include \"h.h\"\n\
               #include \"missing.h\"\n#if 0\nskipped\n#endif\n#ident \"v1\"\n\
               #pragma kiln bake(1)\n#pragma other thing\n_Pragma(\"kiln x\")\n\
               #pragma message(\"hi\")\n#line 50\n";
    let (out, _) = run(src, |pp, interner| {
        pp.set_include_resolver(Box::new(MemoryResolver::new().with_header("h.h", "in_h\n")));
        pp.set_callbacks(Box::new(Spy { log: log.clone(), interner }));
    });
    assert_eq!(out, "1 1\nin_h\n#ident \"v1\"\n#pragma other thing\n#pragma message(\"hi\")\n\n");
    let log = log.borrow();
    let after_main = log.iter().skip_while(|l| *l != "enter from_include=false").skip(1);
    let got: Vec<_> =
        after_main.skip_while(|l| l.starts_with("enter") || l.starts_with("exit")).collect();
    assert_eq!(
        got,
        [
            "define F fn=true",
            "define O fn=false",
            "expand F body=2",
            // The argument is macro-expanded once ([cpp.subst]/1).
            "expand O body=1",
            "undef O had=true",
            "undef nope had=false",
            "include h.h found=true",
            "enter from_include=true",
            "exit",
            "include missing.h found=false",
            "skipped",
            "ident \"v1\"",
            "pragma Directive n=5 consumed=true",
            "pragma Directive n=2 consumed=false",
            // Builtins are reported too, as in clang.
            "expand _Pragma body=0",
            "pragma Operator n=2 consumed=true",
            "message None Message hi",
            "rename None",
        ]
    );
}

/// [cpp.module], [cpp.pragma], [cpp.embed]: out-of-band events in the
/// token stream.
#[test]
fn markers() {
    let interner = Interner::new();
    let sm = SourceManager::new();
    let src = "export module m;\nimport n;\n#pragma weird 1\nint a[] = {\n#embed \"d\"\n};\n";
    let main = sm.add_virtual("t.cpp", src.to_owned()).unwrap();
    let engine = DiagEngine::new(&sm, &DiagOptions::default(), MemorySink::new());
    let mut pp = Preprocessor::new(&sm, &engine, &interner, PpOptions::new(Std::Cxx26));
    pp.set_include_resolver(Box::new(MemoryResolver::new().with_resource("d", b"\x01\x02")));
    pp.enter_main_file(main);
    let mut seen = String::new();
    loop {
        match pp.next_item() {
            PpItem::Token(t) if t.kind == TokKind::Eof => break,
            PpItem::Token(t) => {
                // `#`: the token comes from the embedded resource (the
                // separating commas included).
                let _ = write!(
                    seen,
                    "{}{} ",
                    if pp.is_embed_byte(&t) { "#" } else { "" },
                    pp_spell(&sm, &t)
                );
            }
            PpItem::Marker(Marker::Module { export, import, .. }) => {
                let _ = write!(seen, "[module export={export} import={import}] ");
            }
            PpItem::Marker(Marker::Pragma { tokens, .. }) => {
                let _ = write!(seen, "[pragma {}] ", tokens.len());
            }
            PpItem::Marker(Marker::Embed { .. }) => seen.push_str("[embed] "),
            PpItem::Marker(_) => {}
        }
    }
    assert_eq!(
        seen,
        "[module export=true import=false] export module m ; [module export=false import=true] import n ; \
         [pragma 2] int a [ ] = { [embed] #1 #, #2 } ; "
    );
}

fn pp_spell(sm: &SourceManager, t: &kiln_lex::Token) -> String {
    sm.span_text(t.span).map_or_else(|| "?".to_owned(), str::to_owned)
}

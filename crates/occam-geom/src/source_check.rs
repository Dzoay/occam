//! Source: original
//! What the seam promises about its own source (S4): a family is named only in its own module, and nothing recovers a
//! concrete type. A new family edits no list, so the families are found by their `impl` lines. Tests are not held to
//! it, and neither is the analytic vocabulary, which names forms rather than families. A wasm test cannot read files.
#![cfg(not(target_family = "wasm"))]

use std::path::Path;

/// What recovers a concrete type from a trait object, or reinterprets one: these anywhere, and `Any` as a word.
const RECOVERING: [&str; 3] = ["downcast", "TypeId", "transmute"];

/// The library code of a file: the lines before its test module, without comments.
fn library_code(text: &str) -> Vec<&str> {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines.windows(2).position(|pair| pair == ["#[cfg(test)]", "mod tests {"]).unwrap_or(lines.len());
    lines.into_iter().take(end).filter(|line| !line.trim_start().starts_with("//")).collect()
}

/// Whether `word` appears in `line` as a whole identifier, other than as a variant of the analytic vocabulary.
fn names(line: &str, word: &str) -> bool {
    let is_ident = |c: char| c.is_alphanumeric() || c == '_';
    line.match_indices(word).any(|(at, _)| {
        let (before, rest) = line.split_at(at);
        let after = rest.strip_prefix(word).and_then(|r| r.chars().next());
        !before.ends_with(is_ident) && !after.is_some_and(is_ident) && !before.ends_with("Analytic::")
    })
}

/// The type a family `impl` line implements a family trait for.
fn family_of(line: &str) -> Option<&str> {
    let line = line.trim_start();
    let traits = ["Parametric", "ParametricCurve", "ParametricParamCurve"];
    let rest = traits.iter().find_map(|name| {
        let (head, generic) = (format!("impl {name} for "), format!("> {name} for "));
        line.strip_prefix(head.as_str())
            .or_else(|| line.strip_prefix("impl<").and_then(|r| r.split_once(generic.as_str()).map(|(_, t)| t)))
    })?;
    rest.split(|c: char| !(c.is_alphanumeric() || c == '_')).next().filter(|name| !name.is_empty())
}

/// What breaks the promise in these files, each a path and its library code: an empty list if nothing does.
fn violations(files: &[(String, Vec<&str>)]) -> Vec<String> {
    let families: Vec<(&str, &str)> = files
        .iter()
        .flat_map(|(path, code)| code.iter().filter_map(|line| family_of(line)).map(move |name| (name, path.as_str())))
        .collect();
    let mut found = Vec::new();
    for (path, code) in files {
        for line in code {
            let any = names(line, "Any").then_some("Any");
            for word in RECOVERING.into_iter().filter(|word| line.contains(word)).chain(any) {
                found.push(format!("{path} recovers a type with {word}: {}", line.trim()));
            }
            if path.ends_with("analytic.rs") || (path.ends_with("lib.rs") && line.starts_with("pub use ")) {
                continue;
            }
            for (family, home) in families.iter().filter(|(family, home)| home != path && names(line, family)) {
                found.push(format!("{path} names {family}, whose module is {home}: {}", line.trim()));
            }
        }
    }
    found
}

/// The modules `lib.rs` declares for tests alone.
fn test_modules(lib: &str) -> Vec<String> {
    let lines: Vec<&str> = lib.lines().collect();
    lines
        .windows(2)
        .filter(|pair| pair[0] == "#[cfg(test)]")
        .filter_map(|pair| pair[1].strip_prefix("mod ").and_then(|m| m.strip_suffix(';')).map(str::to_string))
        .collect()
}

/// Every Rust file under `dir`, as its path from `src` and its text.
fn sources(dir: &Path, src: &Path, out: &mut Vec<(String, String)>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            sources(&path, src, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            let name = path.strip_prefix(src).unwrap().to_string_lossy().replace('\\', "/");
            out.push((name, std::fs::read_to_string(&path).unwrap()));
        }
    }
}

#[test]
fn no_family_is_named_outside_its_module_and_nothing_recovers_a_type() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut texts = Vec::new();
    sources(&src, &src, &mut texts);
    let test_only = test_modules(&std::fs::read_to_string(src.join("lib.rs")).unwrap());
    let library =
        |path: &str| !test_only.iter().any(|m| path == format!("{m}.rs") || path.starts_with(&format!("{m}/")));
    let files: Vec<(String, Vec<&str>)> =
        texts.iter().filter(|(path, _)| library(path)).map(|(path, text)| (path.clone(), library_code(text))).collect();
    let found: Vec<&str> = files.iter().flat_map(|(_, code)| code.iter().filter_map(|line| family_of(line))).collect();
    assert!(
        found.contains(&"Plane") && found.contains(&"Line") && found.contains(&"UvCurve"),
        "the families: {found:?}"
    );
    assert!(
        test_only.contains(&"conformance".to_string())
            && files.iter().all(|(path, _)| !path.starts_with("conformance"))
    );
    let violations = violations(&files);
    assert!(violations.is_empty(), "{violations:#?}");
}

#[test]
fn a_family_named_outside_its_module_is_found() {
    let files = vec![
        ("plane.rs".to_string(), vec!["impl Parametric for Plane {"]),
        ("lib.rs".to_string(), vec!["pub use plane::Plane;", "fn f() -> Plane {"]),
        ("analytic.rs".to_string(), vec!["    Plane {"]),
        ("intersect.rs".to_string(), vec!["Analytic::Plane { .. } => 1, // Planes", "let planes = Planer;"]),
        ("wrap.rs".to_string(), vec!["impl<S: Parametric> Parametric for Offset<S> {", "Offset(Plane)"]),
    ];
    assert_eq!(
        violations(&files),
        [
            "lib.rs names Plane, whose module is plane.rs: fn f() -> Plane {",
            "wrap.rs names Plane, whose module is plane.rs: Offset(Plane)",
        ]
    );
}

#[test]
fn recovering_a_concrete_type_is_found() {
    let files = vec![(
        "seam.rs".to_string(),
        vec!["s.downcast_ref::<X>()", "let id = TypeId::of::<X>();", "&dyn Any", "Anyway"],
    )];
    let found = violations(&files);
    assert_eq!(found.len(), 3, "{found:#?}");
    assert!(found[0].contains("downcast") && found[1].contains("TypeId") && found[2].contains("Any"));
}

#[test]
fn library_code_stops_at_the_tests_and_skips_comments() {
    let text = "use a;\n// Plane\n  /// Plane\nfn f() {}\n#[cfg(test)]\nmod tests {\nfn g() {}\n}\n";
    assert_eq!(library_code(text), ["use a;", "fn f() {}"]);
    assert_eq!(test_modules("mod a;\n#[cfg(test)]\nmod b;\nmod c;\n"), ["b"]);
    assert_eq!(family_of("impl ParametricCurve for Line {"), Some("Line"));
    assert_eq!(family_of("impl<C: ParametricCurve> ParametricCurve for Wrong<C> {"), Some("Wrong"));
    assert_eq!(family_of("impl<S: Parametric> Surface for S {"), None);
    assert_eq!(family_of("impl<C: ParametricCurve> ParametricParamCurve for UvCurve<C> {"), Some("UvCurve"));
    assert_eq!(family_of("impl<P: ParametricParamCurve> ParamCurve for P {"), None);
}

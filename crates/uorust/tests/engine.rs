//! The engine against its fixture, against this workspace, and against planted
//! defects (`UE-01` .. `UE-17`).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use uorust::{Options, UorustError};

const STAMP: &str = "2026-11-11T00:00:00Z";

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/uorust is two below the root")
        .to_path_buf()
}

fn fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/thermostat")
}

fn golden() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/thermostat-prism")
}

fn options(name: &str) -> Options {
    Options {
        name: name.to_string(),
        last_modified: STAMP.to_string(),
        advisories: None,
        mir: None,
    }
}

/// A projection; this workspace's runs the MIR path on its committed verdict
/// register, as `just register` and `just uorust` do.
fn project(root: &Path, name: &str) -> BTreeMap<String, Vec<u8>> {
    let mut options = options(name);
    if root == self::root() {
        options.mir = Some(mir_options(
            dumped(root, name),
            verdicts(&root.join("lift-verdicts.toml")),
            uorust::mir::Trial::Off,
        ));
    }
    uorust::project_workspace(root, &options).expect("projects")
}

/// The MIR of the workspace at `root`, dumped once per test run.
fn dumped(root: &Path, name: &str) -> PathBuf {
    static DUMPS: std::sync::Mutex<BTreeMap<String, PathBuf>> =
        std::sync::Mutex::new(BTreeMap::new());
    let mut dumps = DUMPS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    dumps
        .entry(name.to_string())
        .or_insert_with(|| {
            let out = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!("mir-{name}"));
            uorust::mir::dump(root, &out).expect("the MIR dumps");
            out
        })
        .clone()
}

fn verdicts(path: &Path) -> uorust::mir::verdict::Verdicts {
    uorust::mir::verdict::Verdicts::read(path).expect("the verdict register parses")
}

fn mir_options(
    dumps: PathBuf,
    verdicts: uorust::mir::verdict::Verdicts,
    trial: uorust::mir::Trial,
) -> uorust::mir::MirOptions {
    uorust::mir::MirOptions {
        dumps,
        verdicts,
        trial,
        learned: BTreeMap::new(),
    }
}

/// The frontier without its lifted rows, which come last: what did not lift.
fn unlifted(files: &BTreeMap<String, Vec<u8>>) -> String {
    let frontier = text(files, "frontier.toml");
    frontier
        .split("\n[[lifted]]")
        .next()
        .unwrap_or_default()
        .to_string()
}

/// The rows of the array of tables `table` in a projection's frontier.
fn frontier_rows(files: &BTreeMap<String, Vec<u8>>, table: &str) -> Vec<toml::Value> {
    let frontier: toml::Value =
        toml::from_str(&text(files, "frontier.toml")).expect("frontier parses");
    frontier
        .get(table)
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default()
}

/// The `[[refusal]]` rows of `reason` in a projection's frontier, and the
/// count the work queue's `[[reason]]` row of that reason carries: read as
/// rows, so a reason text in the queue never counts as a refusal and a
/// refusal row is never stood in for by the queue's summary.
fn refused(files: &BTreeMap<String, Vec<u8>>, reason: &str) -> (usize, Option<i64>) {
    let of = |row: &toml::Value| row.get("reason").and_then(|v| v.as_str()) == Some(reason);
    let rows = frontier_rows(files, "refusal")
        .iter()
        .filter(|r| of(r))
        .count();
    let queued: Vec<i64> = frontier_rows(files, "reason")
        .iter()
        .filter(|r| of(r))
        .map(|r| {
            r.get("refusals")
                .and_then(|v| v.as_integer())
                .expect("a queue row counts its refusals")
        })
        .collect();
    assert!(queued.len() <= 1, "{reason} is queued twice: {queued:?}");
    (rows, queued.first().copied())
}

/// The reasons of the `[[residue]]` rows of `function`.
fn residue_reasons(files: &BTreeMap<String, Vec<u8>>, function: &str) -> Vec<String> {
    frontier_rows(files, "residue")
        .iter()
        .filter(|r| r.get("function").and_then(|v| v.as_str()) == Some(function))
        .filter_map(|r| r.get("reason").and_then(|v| v.as_str()))
        .map(str::to_string)
        .collect()
}

fn text(files: &BTreeMap<String, Vec<u8>>, path: &str) -> String {
    String::from_utf8(files.get(path).cloned().unwrap_or_default()).expect("utf-8")
}

/// A scratch workspace, written fresh for one test.
fn scratch(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("uorust-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for (path, body) in files {
        let path = dir.join(path);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(path, body).expect("write");
    }
    dir
}

#[test]
fn the_fixture_projects_to_the_committed_project_ue_01() {
    let files = project(&fixture(), "thermostat");
    if let Err(error) = uorust::check(&golden(), &files) {
        panic!(
            "{error}\nregenerate with: cargo run -p uorust -- project \
             crates/uorust/tests/fixtures/thermostat --out \
             crates/uorust/tests/fixtures/thermostat-prism --name thermostat \
             --last-modified {STAMP}"
        );
    }
    // And nothing stale lingers beside it: every committed file is projected.
    let manifest = std::fs::read_to_string(golden().join(uorust::MANIFEST)).expect("manifest");
    assert_eq!(manifest.lines().count() + 1, files.len());
}

#[test]
fn projections_are_byte_identical_ue_02() {
    for (root, name) in [(fixture(), "thermostat"), (root(), "uorust")] {
        let first = project(&root, name);
        let second = project(&root, name);
        assert_eq!(first, second, "{name} projects differently twice");
        assert!(first.contains_key("src/Architecture.lex.tex"));
    }
}

#[test]
fn ambiguity_is_refused_with_a_typed_open_claim_ue_03() {
    let dir = scratch(
        "ue03",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"amb\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
            ),
            (
                "src/lib.rs",
                "//! Planted defects.\npub mod a;\npub mod b;\npub use a::*;\n\n/// Calls go.\npub fn run() -> u8 {\n    go()\n}\n\n/// Picks an implementation by its argument's type.\npub fn pick(p: a::Pair) -> a::Wide {\n    a::Wide::from(p.0)\n}\n",
            ),
            (
                "src/a.rs",
                "/// One.\npub fn go() -> u8 {\n    1\n}\n\n/// A byte.\npub struct Pair(pub u8);\n\n/// A wide value.\npub struct Wide(pub u32);\n\nimpl From<u8> for Wide {\n    fn from(x: u8) -> Wide {\n        Wide(u32::from(x))\n    }\n}\n\nimpl From<u16> for Wide {\n    fn from(x: u16) -> Wide {\n        Wide(u32::from(x) + 1)\n    }\n}\n\n/// Both a state and an error.\n#[derive(Debug)]\npub enum LoadState {\n    /// Idle.\n    Idle,\n    /// Done.\n    Done,\n}\n\nimpl std::fmt::Display for LoadState {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        write!(f, \"{self:?}\")\n    }\n}\n\nimpl std::error::Error for LoadState {}\n",
            ),
            ("src/b.rs", "/// Two.\npub fn go() -> u8 {\n    2\n}\n"),
        ],
    );
    let files = project(&dir, "amb");
    let frontier = text(&files, "frontier.toml");
    // Each reason from a refusal row of its own, an open claim; the queue's
    // [[reason]] counts alone do not show one.
    let refusals: Vec<&str> = frontier
        .split("\n[[")
        .filter_map(|row| row.strip_prefix("refusal]]\n"))
        .collect();
    for reason in [
        "ambiguous-call",
        "ambiguous-fact",
        "feature-without-acceptance",
        "no-msrv",
    ] {
        assert!(
            refusals.iter().any(
                |row| row.lines().any(|l| l == format!("reason = \"{reason}\""))
                    && row.lines().any(|l| l == "status = \"open\"")
            ),
            "{reason} missing:\n{frontier}"
        );
    }
    // The ambiguous call is the one the written code leaves open: `p.0` is
    // a field, whose type the resolution does not read, and both `From`
    // implementations of the workspace can take it.
    let ambiguous = refusals
        .iter()
        .find(|row| row.contains("reason = \"ambiguous-call\""))
        .expect("an ambiguous-call row");
    assert!(
        ambiguous.contains("span = \"src/lib.rs:12\"")
            && ambiguous.contains("Wide::from")
            && ambiguous.contains("src/a.rs:13")
            && ambiguous.contains("src/a.rs:19"),
        "{ambiguous}"
    );
    let interface = text(&files, "src/Interface.lex.tex");
    assert!(
        !interface.contains("Foundation.Arch::Feature {"),
        "an unaccepted feature was emitted:\n{interface}"
    );
    let behavior = text(&files, "src/Behavior.lex.tex");
    assert!(!behavior.contains("LoadStateFailure"), "{behavior}");
    assert!(!text(&files, "src/Design.lex.tex").contains("LoadState"));
    let quality = text(&files, "src/Quality.lex.tex");
    assert!(!quality.contains("MinimumRustMinor"));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_call_admission_resolves_lifts_as_the_function_it_resolves_to() {
    // `go` of the root module is `a::go`, through the glob import: one
    // function by the language's name resolution, which admission and the
    // lift both read it by.
    let dir = scratch(
        "glob-call",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"amb\"\nversion = \"0.1.0\"\nedition = \"2021\"\nrust-version = \"1.70\"\n\n[workspace]\n",
            ),
            (
                "src/lib.rs",
                "//! A glob import.\npub mod a;\npub mod b;\npub use a::*;\n\n/// Calls go.\npub fn run() -> u8 {\n    go()\n}\n",
            ),
            ("src/a.rs", "/// One.\npub fn go() -> u8 {\n    1\n}\n"),
            ("src/b.rs", "/// Two.\npub fn go() -> u8 {\n    2\n}\n"),
        ],
    );
    let files = project(&dir, "amb");
    let _ = std::fs::remove_dir_all(&dir);
    let frontier = text(&files, "frontier.toml");
    assert!(!frontier.contains("ambiguous-call"), "{frontier}");
    assert!(
        frontier.contains("[lift]\nfunctions = 3\nlifted = 3\nresidue = 0\n"),
        "{frontier}"
    );
    let behavior = text(&files, "src/Behavior.lex.tex");
    let run = behavior
        .split("definition{")
        .find(|d| d.contains("name: run,"))
        .unwrap_or_else(|| panic!("run is not lifted:\n{behavior}"));
    assert!(run.contains("body: (aGo)}"), "run calls another go:\n{run}");
}

#[test]
fn the_floor_lifts_and_the_rest_is_typed_residue_ue_04() {
    let files = project(&fixture(), "thermostat");
    let behavior = text(&files, "src/Behavior.lex.tex");
    for name in ["shouldHeat", "withinBand", "nextMode", "halt"] {
        assert!(
            behavior.contains(&format!("name: {name},")),
            "{name}:\n{behavior}"
        );
    }
    assert!(behavior.contains("statement: ((shouldHeat 18 21) = true),"));
    assert!(behavior.contains("statement: ((withinBand 10 5 9) = false),"));
    assert!(behavior.contains("proof: decide{}"));
    assert!(behavior.contains("(prim quotient (low + high) 2 0 : Nat)"));
    let frontier = text(&files, "frontier.toml");
    // Arithmetic whose meaning is a build setting stays typed residue.
    let planted = scratch(
        "arith",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"arith\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "/// Overflows.\npub fn mid(a: u32, b: u32) -> u32 {\n    (a + b) / 2\n}\n",
            ),
        ],
    );
    let planted_files = project(&planted, "arith");
    assert_eq!(
        residue_reasons(&planted_files, "arith::mid"),
        ["arithmetic"],
        "{}",
        text(&planted_files, "frontier.toml")
    );
    let _ = std::fs::remove_dir_all(planted);
    // The I/O shell is an effect row, not residue: `load_target` reads a file
    // and `target_from_env` reads the environment, directly or through it.
    let effect = |function: &str, needle: &str| {
        frontier
            .split("[[effect]]")
            .skip(1)
            .any(|row| row.contains(&format!("function = \"{function}\"")) && row.contains(needle))
    };
    assert!(
        effect(
            "thermo_core::config::load_target",
            "Fs effect: fs::read_to_string"
        ),
        "{frontier}"
    );
    assert!(
        residue_reasons(&files, "thermo_core::config::load_target").is_empty(),
        "{frontier}"
    );
}

#[test]
fn every_failure_is_a_typed_variant_ue_06() {
    let missing = std::env::temp_dir().join("uorust-ue06-does-not-exist");
    assert!(matches!(
        uorust::project_workspace(&missing, &options("x")),
        Err(UorustError::Io { .. })
    ));

    let empty = scratch("ue06-empty", &[("README", "no manifest\n")]);
    assert!(matches!(
        uorust::project_workspace(&empty, &options("x")),
        Err(UorustError::CargoMetadata(_))
    ));
    assert!(matches!(
        uorust::commit_time(&empty),
        Err(UorustError::NoTimestamp)
    ));

    let bad_lock = scratch(
        "ue06-lock",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"l\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n",
            ),
            ("src/lib.rs", "//! L.\n"),
            ("Cargo.lock", "this is = = not toml\n"),
        ],
    );
    assert!(matches!(
        uorust::project_workspace(&bad_lock, &options("x")),
        Err(UorustError::Malformed { .. })
    ));

    let bad_stamp = Options {
        name: "x".into(),
        last_modified: "yesterday".into(),
        advisories: None,
        mir: None,
    };
    assert!(matches!(
        uorust::project_workspace(&fixture(), &bad_stamp),
        Err(UorustError::BadTimestamp(_))
    ));

    let foreign = scratch("ue06-foreign", &[("notes.txt", "mine\n")]);
    let files = project(&fixture(), "thermostat");
    assert!(matches!(
        uorust::write(&foreign, &files),
        Err(UorustError::ForeignOutput(_))
    ));

    let out = std::env::temp_dir().join(format!("uorust-ue06-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    uorust::write(&out, &files).expect("writes into an absent directory");
    uorust::write(&out, &files).expect("rewrites its own projection");
    uorust::check(&out, &files).expect("a fresh write is current");
    std::fs::write(out.join("src/Behavior.lex.tex"), "planted\n").expect("plant");
    match uorust::check(&out, &files) {
        Err(UorustError::Stale(paths)) => assert_eq!(paths, vec!["src/Behavior.lex.tex"]),
        other => panic!("expected Stale, got {other:?}"),
    }
    for dir in [empty, bad_lock, foreign, out] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

#[test]
fn vv_runs_the_sdk_checks_on_both_projections_ue_07() {
    let justfile = std::fs::read_to_string(root().join("Justfile")).expect("Justfile");
    let vv = justfile
        .lines()
        .find(|l| l.starts_with("vv:"))
        .expect("a vv recipe");
    assert!(vv.split_whitespace().any(|w| w == "uorust"), "{vv}");
    let recipe: String = justfile
        .split("\nuorust:\n")
        .nth(1)
        .expect("a uorust recipe")
        .lines()
        .take_while(|l| l.starts_with("    ") || l.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    for needle in [
        "check crates/uorust/tests/fixtures/thermostat",
        "cd crates/uorust/tests/fixtures/thermostat-prism && lexlean fmt --check && lexlean lock --check && lexlean verify && prismpm check",
        "project . --out target/uorust-self",
        "cd target/uorust-self && lexlean fmt --check && lexlean lock --check && lexlean verify && prismpm check",
        "--advisories crates/uorust/tests/fixtures/advised/advisory-db",
        "cd target/uorust-advised && lexlean fmt --check && lexlean lock --check && lexlean verify && prismpm check",
    ] {
        assert!(recipe.contains(needle), "the uorust recipe lacks `{needle}`:\n{recipe}");
    }
}

#[test]
fn pub_trait_methods_are_interface_calls_ue_08() {
    let files = project(&fixture(), "thermostat");
    let interface = text(&files, "src/Interface.lex.tex");
    for call in ["Read", "Calibrated"] {
        assert!(
            interface.contains(&format!("name: callThermoCoreSensorSensor{call},")),
            "{interface}"
        );
    }
    let feature = interface
        .split("definition{")
        .find(|d| d.contains("name: featureThermoCoreSensor,"))
        .expect("the sensor module is a feature");
    assert!(feature.contains("\"Interface::callThermoCoreSensorSensorRead\""));
    assert!(feature.contains("\"Interface::callThermoCoreSensorSensorCalibrated\""));
    assert!(
        feature.contains("\"Interface::acThermoCoreThermoCoreSensorAFixedSensorReadsItsValue\""),
        "{feature}"
    );
}

#[test]
fn each_module_is_an_opm_view_ue_09() {
    let files = project(&fixture(), "thermostat");
    let design = text(&files, "src/Design.lex.tex");
    for view in [
        "opmModuleLibThermoCore",
        "opmModuleLibThermoCoreConfig",
        "opmModuleLibThermoCoreSensor",
        "opmModuleBinThermo",
    ] {
        assert!(design.contains(&format!("name: {view},")), "{view}");
    }
    let zoom = design
        .split("definition{")
        .find(|d| d.contains("name: opmModuleLibThermoCoreConfigZoom"))
        .expect("load_target zooms into parse_target");
    assert!(zoom.contains("process := \"Design::processThermoCoreConfigLoadTarget\""));
    assert!(zoom.contains("subprocess := \"Design::processThermoCoreConfigParseTarget\""));
    assert!(design.contains("name: opmModuleLibThermoCoreConfigInvokesThermoCoreConfigLoadTargetThermoCoreConfigParseTarget,"));
}

#[test]
fn engine_modules_have_their_own_titles_ue_10() {
    let files = project(&fixture(), "thermostat");
    for (module, title) in [
        ("src/Behavior.lex.tex", "Lifted Rust behavior"),
        ("src/SupplyChain.lex.tex", "Locked supply chain"),
    ] {
        let source = text(&files, module);
        assert!(source.contains("\\useglossary{uorust.titles@1.0.0}"));
        assert!(source.contains(&format!("\\title{{{title}}}")), "{source}");
    }
    for path in ["lexlean.toml", "lexlean.lock"] {
        assert!(text(&files, path).contains("language/uorust.titles"));
    }
    let surfaces: String = ["behavior", "supply-chain"]
        .iter()
        .map(|e| text(&files, &format!("language/uorust.titles/entries/{e}.toml")))
        .collect();
    assert!(surfaces.contains("surface = \"Lifted Rust behavior\""));
    assert!(surfaces.contains("surface = \"Locked supply chain\""));
}

#[test]
fn the_widened_lift_covers_the_fixture_ue_11() {
    let files = project(&fixture(), "thermostat");
    let behavior = text(&files, "src/Behavior.lex.tex");
    for (name, needle) in [
        (
            "width",
            "(prim subtract (project band high) (project band low) : Nat)",
        ),
        ("checkedMidpoint", "<? 4294967296)"),
        ("tick", "(prim remainder (counter + 1) 256 0 : Nat)"),
        (
            "shortfall",
            "(prim subtract (project band low) current : Nat)",
        ),
        ("freezing", "(prim less_than (0 : Int64) outdoor : Bool)"),
        (
            "delta",
            "(prim checked_subtract after before : (Option Int64))",
        ),
        ("checkedValue", "(record TupleBandBool {item0 := band"),
        ("isCelsius", "(prim equal unit \"C\" : Bool)"),
        ("holdsOff", "mutual: holdsOffMutual"),
        (
            "steady",
            "(if coldArg then (if occupied then false else true)",
        ),
        ("hasLine", "(let lines : (List String) := (nil String);"),
        ("phasesOff", "measure: (size_of_list_Phase phases)"),
        (
            "phaseOff",
            "(match phase | Phase.mk label step => (holdsOff step))",
        ),
        (
            "clamped",
            "(let state1 : TupleBoolNat := (if (value <? (project band low))",
        ),
        ("above", "recursive_argument: readings"),
        ("firstReading", "(prim index readings 0 : (Option Nat))"),
        ("targetOr", "| Option.some"),
        ("heatFor", "(ctor Result.error [Bool, ConfigError]"),
        ("fanLabel", "(if (speed =? 0) then \"off\""),
        (
            "labelled",
            "(prim join (prim append (cons value (nil String)) (cons \" \" (cons unit (nil String))) : (List String)) \"\" : String)",
        ),
    ] {
        let decl = behavior
            .split("definition{")
            .find(|d| d.contains(&format!("name: {name},")))
            .unwrap_or_else(|| panic!("{name} is lifted:\n{behavior}"));
        assert!(decl.contains(needle), "{name} lacks {needle}:\n{decl}");
    }
    assert!(behavior.contains("name: Band,"));
    assert!(behavior.contains("statement: ((freezing (-3 : Int64)) = true),"));
    assert!(behavior.contains("axioms: [propext]"));
    let frontier = unlifted(&files);
    assert!(!frontier.contains("function = \"thermo_core::midpoint\""));
    assert!(
        !frontier.contains("function = \"thermo_core::band::"),
        "{frontier}"
    );
}

#[test]
fn iterator_chains_methods_and_maps_lift_ue_15() {
    let files = project(&fixture(), "thermostat");
    let behavior = text(&files, "src/Behavior.lex.tex");
    let decl = |name: &str| -> String {
        behavior
            .split("definition{")
            .find(|d| d.contains(&format!("name: {name},")))
            .unwrap_or_else(|| panic!("{name} is lifted:\n{behavior}"))
            .to_string()
    };
    for (name, needle) in [
        ("roomIsCold", "parameters: [selfValue : Room, floor : Nat]"),
        ("roomReread", "result: Room,"),
        ("roomLabel", "then \"unnamed\" else"),
        ("namesMap1", "recursive_argument: items"),
        ("names", "(namesMap1 rooms)"),
        ("coldFilter1", "(roomIsCold room floor)"),
        ("anyColdAny1", "then true else"),
        ("allWithinAll1", "else false"),
        ("firstColdFind1", "(ctor Option.some [Room] room)"),
        ("highestFold1", "| List.nil => best"),
        (
            "coldCount",
            "(prim length (coldCountFilter1 rooms floor) : Nat)",
        ),
        ("scheduledContains1", "(prim equal item"),
        ("roll", "(prim join (names rooms) \", \" : String)"),
        ("pairs", "axioms: [Classical.choice, Quot.sound, propext]"),
        ("targetOf", "| Option.none => 18"),
        ("roomsOf", "(List TupleStringNat)"),
        ("hasTarget", "(hasTargetHas1 targets room)"),
        ("closed", "(closedContains1 rooms room)"),
        ("labelWidth", "(prim length label : Nat)"),
        ("clamp", "(let low : Nat := (project pair"),
        ("blank", "body: \"\""),
        ("nextMode", "coldArg : Bool"),
        ("coldNamesLoop1", "recursive_argument: items"),
        ("coldNames", "(coldNamesLoop1 rooms"),
        ("reportLoop1", "(record TupleStringNat {item0 := "),
        ("report", "(reportLoop1 rooms (record TupleStringNat"),
        ("readingsOfLoop1", "(prim append state"),
        // A mutual family: a struct joins its enum as an inductive of one
        // constructor, read through accessors, and recursion over it is
        // well-founded on its size.
        ("phaseLabel", "(field_Phase_label phase)"),
        ("hold", "(ctor Phase.mk label (ctor Step.Hold target))"),
        ("size_of_Step", "mutual: SizeOfPhase"),
        ("occupiedTarget", "measure: (size_of_Step step)"),
        ("isFinal", "| Menu.Choose"),
        // A lifetime parameter is erased.
        ("labelOr", "parameters: [phase : Phase, fallback : String]"),
        // An ordered set or map is built by a sorted insert.
        ("labels", "(labelsLoop1 phases out)"),
        (
            "labelsInsert1",
            "then (cons head1 rest1) else (if (prim less_than item1 head1 : Bool)",
        ),
        ("heldTargetsInsert1", "then (cons item1 rest1) else"),
        // format! holes of naturals, booleans and constants.
        ("describe", "(Prelude::natDecimal target)"),
        ("describe", "(if sequence then \"true\" else \"false\")"),
        ("describe", "(cons (heldUnit) "),
        // A while loop is a walk measured by its bound less its cursor,
        // with the theorem that each step decreases it.
        (
            "everyOtherLoop1",
            "measure: (prim subtract (prim length rooms : Nat) at1 : Nat)",
        ),
        (
            "everyOtherLoop1",
            "(everyOtherLoop1 (at1 + 2) (everyOtherStep1 at1 state1 rooms) rooms)",
        ),
        ("firstRoomsLoop1", "evidence: [decreases_firstRoomsLoop1_0]"),
        // A family held in an option of a struct of a list is measured through it.
        ("size_of_option_Floor", "(1 + (size_of_Floor x0))"),
        ("annexedRoom", "measure: (size_of_Plan plan)"),
        // A function parameter is applied; a closure passed for one is its
        // lambda, with the captures it reads.
        ("renamedMap1", "(apply name room)"),
        ("prefixed", "(fun [prefix] (room : String) =>"),
        // `m.entry(k).or_default().push(v)` reads the entry, updates it
        // and inserts it back.
        (
            "byReadingPart1",
            "(match (byReadingGet1 state1 (project room reading))",
        ),
        // A struct written through the crate's modules is the crate's.
        (
            "openingBand",
            "(record Band {low := (project room reading), high := target})",
        ),
        // `()` is LexLean's Unit.
        ("validate", "(ctor Result.ok [Unit, String] ())"),
    ] {
        let d = decl(name);
        assert!(d.contains(needle), "{name} lacks {needle}:\n{d}");
    }
    let frontier = unlifted(&files);
    assert!(!frontier.contains("thermo_core::roster::"), "{frontier}");
    assert!(!frontier.contains("thermo_core::schedule::"), "{frontier}");
    // A pair that names a data enum is a product, which the enum may hold.
    assert!(behavior.contains("{name: Submenu, fields: [(List (String * Menu))]}"));
    assert!(behavior.contains("statement: ((step = (ctor Step.Either occupied rest)) -> ((size_of_Step occupied) < (size_of_Step step)))"));
}

#[test]
fn advisories_are_risks_on_the_supply_chain_ue_12() {
    let advised = root().join("crates/uorust/tests/fixtures/advised");
    let db = advised.join("advisory-db");
    let with_db = Options {
        advisories: Some(db.clone()),
        ..options("advised")
    };
    let files = uorust::project_workspace(&advised, &with_db).expect("projects");
    let security = text(&files, "src/Security.lex.tex");
    for id in ["20180018", "20190009", "20190012", "20210003"] {
        assert!(
            security.contains(&format!("name: riskAdvisoryRustsec{id}V0p6p9,")),
            "{id}:\n{security}"
        );
    }
    // 0.6.9 is patched against RUSTSEC-2018-0003.
    assert!(!security.contains("Rustsec20180003"));
    // deny.toml ignores RUSTSEC-2018-0018 with a reason: an accepted risk, no control.
    assert!(security.contains("name: acceptedThreatAdvisoryRustsec20180018V0p6p9,"));
    assert!(!security.contains("name: controlAdvisoryRustsec20180018V0p6p9,"));
    // The others are answered by the advisory gate, which fails today.
    let measurement = security
        .split("definition{")
        .find(|d| d.contains("name: measurementAdvisoryRustsec20190012V0p6p9,"))
        .expect("a measurement");
    assert!(measurement.contains("passed := false"), "{measurement}");
    let frontier = text(&files, "frontier.toml");
    assert!(!frontier.contains("advisories-unconsulted"), "{frontier}");

    // Without a database, the frontier says no advisory was consulted.
    let files = project(&advised, "advised");
    assert_eq!(
        refused(&files, "advisories-unconsulted"),
        (1, Some(1)),
        "{}",
        text(&files, "frontier.toml")
    );
    assert!(!text(&files, "src/Security.lex.tex").contains("Advisory"));

    // Without deny.toml, an advisory nothing answers is refused, not modeled.
    let copy = scratch(
        "ue12",
        &[
            (
                "Cargo.toml",
                &std::fs::read_to_string(advised.join("Cargo.toml")).expect("manifest"),
            ),
            (
                "Cargo.lock",
                &std::fs::read_to_string(advised.join("Cargo.lock")).expect("lock"),
            ),
            (
                "src/lib.rs",
                &std::fs::read_to_string(advised.join("src/lib.rs")).expect("lib"),
            ),
        ],
    );
    let bare = Options {
        advisories: Some(db),
        ..options("advised")
    };
    let files = uorust::project_workspace(&copy, &bare).expect("projects");
    let frontier = text(&files, "frontier.toml");
    // Four refusals, one per advisory, and the work queue counts them.
    assert_eq!(
        refused(&files, "unanswered-advisory"),
        (4, Some(4)),
        "{frontier}"
    );
    assert!(!text(&files, "src/Security.lex.tex").contains("Advisory"));
    let _ = std::fs::remove_dir_all(copy);
}

#[test]
fn every_feature_of_this_workspace_is_accepted_ue_13() {
    let files = project(&root(), "uorust");
    let frontier = text(&files, "frontier.toml");
    assert!(
        !frontier.contains("feature-without-acceptance"),
        "{frontier}"
    );
    let interface = text(&files, "src/Interface.lex.tex");
    // `uorust mir` and the module `uorust::mir` are two features, told apart.
    for feature in [
        "featureRepoModelCodegen",
        "featureXtaskMain",
        "featureUorustMir",
        "featureUorustCommandMir",
    ] {
        assert!(
            interface.contains(&format!("name: {feature},")),
            "{feature}"
        );
    }
}

/// The `definition{...}` records of the given module texts, by name.
fn records(files: impl IntoIterator<Item = String>) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for body in files {
        for chunk in body.split("definition{").skip(1) {
            let first = chunk.lines().next().unwrap_or_default();
            let record = if first.contains("body:") {
                first
            } else {
                chunk.split("\n}").next().unwrap_or_default()
            };
            let Some(name) = record.split("name: ").nth(1) else {
                continue;
            };
            let name: String = name
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect();
            out.insert(name, record.to_string());
        }
    }
    out
}

/// A `key := "value"` field of a rendered record.
fn field<'a>(record: &'a str, key: &str) -> &'a str {
    let start = record
        .find(&format!("{key} := \""))
        .unwrap_or_else(|| panic!("no {key} in {record}"))
        + key.len()
        + 5;
    let end = record[start..].find('"').expect("closing quote");
    &record[start..start + end]
}

#[test]
fn the_self_projection_reproduces_the_hand_model_ue_14() {
    let app = root().join("app/src");
    let hand_files: Vec<String> = std::fs::read_dir(&app)
        .expect("app/src")
        .map(|entry| entry.expect("entry").path())
        .filter(|path| path.to_string_lossy().ends_with(".lex.tex"))
        .map(|path| std::fs::read_to_string(path).expect("hand module"))
        .collect();
    let hand = records(hand_files);
    let files = project(&root(), "uorust");
    let derived = records(
        files
            .iter()
            .filter(|(path, _)| path.starts_with("src/") && path.ends_with(".lex.tex"))
            .map(|(_, bytes)| String::from_utf8(bytes.clone()).expect("utf-8")),
    );

    let table: toml::Value = toml::from_str(
        &std::fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixpoint.toml"),
        )
        .expect("fixpoint.toml"),
    )
    .expect("fixpoint.toml parses");
    let rows = |key: &str| {
        table
            .get(key)
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default()
    };
    let get = |row: &toml::Value, key: &str| -> String {
        row.get(key)
            .and_then(|v| v.as_str())
            .expect(key)
            .to_string()
    };

    // Each pair whose two records do not both state the fact is drift.
    let mut drift = Vec::new();
    let mut accounted = std::collections::BTreeSet::new();
    for row in rows("pair") {
        let (h, d, fact) = (get(&row, "hand"), get(&row, "derived"), get(&row, "fact"));
        accounted.insert(h.clone());
        for (side, name, set) in [("hand", &h, &hand), ("derived", &d, &derived)] {
            match set.get(name) {
                None => drift.push(format!("{side} record {name} is missing")),
                Some(body) if !body.contains(&fact) => {
                    drift.push(format!("{side} record {name} does not state `{fact}`"));
                }
                Some(_) => {}
            }
        }
    }
    for row in rows("outside") {
        let h = get(&row, "hand");
        assert!(
            hand.contains_key(&h),
            "outside row names no hand record: {h}"
        );
        assert!(
            !get(&row, "reason").is_empty(),
            "{h} is outside without a reason"
        );
        // The reason is typed, and what each kind claims is checked.
        let body = &hand[&h];
        match get(&row, "kind").as_str() {
            "external-system" => assert!(
                body.contains("ComponentKind.externalSystem"),
                "{h} is outside as an external system but is not one"
            ),
            "private-stage" => {
                let module = get(&row, "module");
                let lib = std::fs::read_to_string(root().join("crates/uorust/src/lib.rs"))
                    .expect("lib.rs");
                assert!(
                    lib.lines().any(|l| l == format!("mod {module};")),
                    "{h} names {module}, which is not a private module of the library"
                );
            }
            "outside-rust" => {
                let path = root().join(get(&row, "path"));
                assert!(path.exists(), "{h} names a path that does not exist");
                let mut stack = vec![path];
                while let Some(at) = stack.pop() {
                    if at.is_dir() {
                        if at.ends_with("target")
                            || at.file_name().is_some_and(|n| {
                                n.to_string_lossy().starts_with('.') && n != ".github"
                            })
                        {
                            continue;
                        }
                        for entry in std::fs::read_dir(&at).expect("dir") {
                            stack.push(entry.expect("entry").path());
                        }
                    } else {
                        assert!(
                            at.extension().is_none_or(|e| e != "rs"),
                            "{h} is outside the Rust code, but {} is Rust",
                            at.display()
                        );
                    }
                }
            }
            "run-time" => {
                let gate = get(&row, "gate");
                let ids = std::fs::read_to_string(root().join("model/ids.toml")).expect("ids");
                let just = std::fs::read_to_string(root().join("Justfile")).expect("Justfile");
                assert!(
                    ids.contains(&format!("id = \"{gate}\""))
                        || just.lines().any(|l| l.starts_with(&format!("{gate}:"))),
                    "{h} names a gate, {gate}, that is neither a register ID nor a recipe"
                );
            }
            "reviewer-judgement" => assert!(
                h.starts_with("modeledThreat") || h.starts_with("modeledControl"),
                "{h} is a reviewer's judgement but not a modeled threat or control"
            ),
            "this-gate" => assert_eq!(h, "measureSelfModelDrift", "{h} is not this gate's measure"),
            other => panic!("{h} is outside with an untyped kind {other}"),
        }
        assert!(
            accounted.insert(h.clone()),
            "{h} is both paired and outside"
        );
    }
    // Every hand record of a checked kind is accounted for, one way or the other.
    let checked: Vec<String> = rows("checked")
        .iter()
        .map(|v| v.as_str().expect("prefix").to_string())
        .collect();
    for name in hand.keys() {
        let kind = checked.iter().find(|prefix| {
            name.strip_prefix(prefix.as_str())
                .and_then(|rest| rest.chars().next())
                .is_some_and(|c| c.is_ascii_uppercase())
        });
        if kind.is_some() && !accounted.contains(name) {
            drift.push(format!("hand record {name} is neither paired nor outside"));
        }
    }

    // The route table: the hand model's `routes`, in order, is uorust::ROUTES.
    let routes = &hand.get("routes").expect("hand routes")[..];
    let hand_routes: Vec<[String; 4]> = routes
        .split("record Route {")
        .skip(1)
        .map(|r| {
            let r = r.split('}').next().unwrap_or_default();
            ["id", "rustFact", "prismRecord", "facet"].map(|k| field(r, k).to_string())
        })
        .collect();
    let code_routes: Vec<[String; 4]> = uorust::ROUTES
        .iter()
        .map(|r| [r.id, r.rust_fact, r.prism_record, r.facet].map(str::to_string))
        .collect();
    if hand_routes != code_routes {
        drift.push(format!(
            "route table differs:\nhand {hand_routes:?}\ncode {code_routes:?}"
        ));
    }

    assert!(
        hand.get("measureSelfModelDrift")
            .is_some_and(|m| m.contains("threshold := 0")),
        "the hand model's drift threshold is 0"
    );
    assert!(
        drift.is_empty(),
        "self-model drift {}:\n{}",
        drift.len(),
        drift.join("\n")
    );
}

#[test]
fn no_lifted_kappa_turns_unlifted_ue_16() {
    use uorust::register::{admit, Register};
    // The incumbent registers: the committed fixture frontier and this
    // workspace's committed lift register.
    for (root, name, incumbent) in [
        (root(), "uorust", root().join("frontier.toml")),
        (fixture(), "thermostat", golden().join("frontier.toml")),
    ] {
        let files = project(&root, name);
        // Typed, keyed by κ, every function once.
        let candidate = uorust::lift_register(&files).expect("the frontier is a register");
        let committed = std::fs::read_to_string(&incumbent).expect("the committed register");
        let incumbent =
            Register::parse(&incumbent, &committed).expect("the committed register parses");
        let regressed = admit(&incumbent, &candidate);
        assert!(
            regressed.is_empty(),
            "{name}: lifted κ decided otherwise:\n{}",
            regressed.join("\n")
        );
        if name == "uorust" {
            assert!(
                committed == candidate.to_text(),
                "frontier.toml is not this workspace's lift register; run `just register`"
            );
        }
        let frontier: toml::Value =
            toml::from_str(&text(&files, "frontier.toml")).expect("frontier parses");
        let rows = |table: &str| {
            frontier
                .get(table)
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default()
        };
        for row in &rows("residue") {
            for field in ["function", "span", "reason", "detail"] {
                assert!(row.get(field).is_some(), "{name}: untyped residue {row}");
            }
        }
        // A divergence is a lifted row's alone, and it says where the
        // definition parts from the Rust function; the register keeps it.
        for table in ["residue", "foreign", "effect"] {
            for row in rows(table) {
                assert!(row.get("divergence").is_none(), "{name}: {row}");
            }
        }
        for row in rows("lifted") {
            if let Some(divergence) = row.get("divergence") {
                assert!(
                    divergence.as_str().is_some_and(|d| !d.trim().is_empty()),
                    "{name}: an empty divergence {row}"
                );
                let function = row.get("function").and_then(|v| v.as_str());
                assert!(
                    candidate
                        .rows
                        .values()
                        .any(|r| Some(r.function.as_str()) == function
                            && r.divergence.as_deref() == divergence.as_str()),
                    "{name}: the register drops the divergence of {row}"
                );
            }
        }
        for row in &rows("foreign") {
            let detail = row
                .get("detail")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(
                detail.contains(" crate with no LexLean model"),
                "{name}: {row}"
            );
        }
        // An effect row names the effect it performs or reaches.
        for row in rows("effect") {
            let effect = row
                .get("effect")
                .and_then(|v| v.as_str())
                .unwrap_or_default();
            assert!(
                effect.contains(" effect: ") || effect.contains("reaches an effect"),
                "{name}: an effect row without its effect: {row}"
            );
        }
    }
}

/// Every function of product code, counted from the source apart from the
/// engine: free functions, inherent and trait-impl methods and trait methods
/// at item level, outside `#[cfg(test)]` modules (inline, or out of line in
/// their own file or directory) and not `#[test]`, in the `src/` tree of
/// every package of the workspace at `root` ([`members`]).
fn product_functions(root: &Path) -> Vec<String> {
    fn cfg_test(attrs: &[syn::Attribute]) -> bool {
        attrs.iter().any(|a| {
            a.path().is_ident("cfg")
                && quote::ToTokens::to_token_stream(a)
                    .to_string()
                    .contains("test")
        })
    }
    fn is_test(attrs: &[syn::Attribute]) -> bool {
        attrs
            .iter()
            .any(|a| a.path().segments.last().is_some_and(|s| s.ident == "test"))
    }
    /// The functions of `list`, and in `tests` the file and the directory of
    /// each out-of-line `#[cfg(test)] mod name;`, which lie in `dir`.
    fn items(
        list: &[syn::Item],
        at: &str,
        dir: &Path,
        out: &mut Vec<String>,
        tests: &mut Vec<PathBuf>,
    ) {
        for item in list {
            match item {
                syn::Item::Fn(f) if !is_test(&f.attrs) => out.push(format!("{at}{}", f.sig.ident)),
                syn::Item::Impl(i) => {
                    for m in &i.items {
                        if let syn::ImplItem::Fn(f) = m {
                            if !is_test(&f.attrs) {
                                out.push(format!("{at}impl {}", f.sig.ident));
                            }
                        }
                    }
                }
                syn::Item::Trait(t) => {
                    for m in &t.items {
                        if let syn::TraitItem::Fn(f) = m {
                            out.push(format!("{at}trait {}", f.sig.ident));
                        }
                    }
                }
                syn::Item::Mod(m) if !cfg_test(&m.attrs) => {
                    if let Some((_, inner)) = &m.content {
                        items(inner, at, &dir.join(m.ident.to_string()), out, tests);
                    }
                }
                syn::Item::Mod(m) if m.content.is_none() => {
                    assert!(
                        !m.attrs.iter().any(|a| a.path().is_ident("path")),
                        "{at}a #[path] test module this walker does not resolve"
                    );
                    let name = m.ident.to_string();
                    tests.push(dir.join(format!("{name}.rs")));
                    tests.push(dir.join(name));
                }
                _ => {}
            }
        }
    }
    fn walk(dir: &Path, top: bool, out: &mut Vec<PathBuf>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("dir")
            .map(|e| e.expect("entry").path())
            .collect();
        entries.sort();
        if !top && entries.iter().any(|p| p.ends_with("Cargo.toml")) {
            return;
        }
        for path in entries {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if path.is_dir() {
                if !matches!(name, "target" | "fixtures") {
                    walk(&path, false, out);
                }
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    for package in members(root) {
        assert!(
            package.join("src").is_dir(),
            "{}: a member with no src/ tree",
            package.display()
        );
        let mut files = Vec::new();
        walk(&package.join("src"), true, &mut files);
        let (mut found, mut tests) = (Vec::new(), Vec::new());
        for path in &files {
            let text = std::fs::read_to_string(path).expect("source");
            let file = syn::parse_file(&text).expect("parses");
            // A module's children lie beside a crate root or a `mod.rs`, and
            // in the directory named after any other file.
            let parent = path.parent().expect("a directory");
            let stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            let root_file = matches!(stem, "lib" | "main" | "mod") || parent.ends_with("src/bin");
            let dir = if root_file {
                parent.to_path_buf()
            } else {
                parent.join(stem)
            };
            let mut functions = Vec::new();
            items(
                &file.items,
                &format!("{}: ", path.display()),
                &dir,
                &mut functions,
                &mut tests,
            );
            found.push((path, functions));
        }
        for (path, functions) in found {
            if !tests.iter().any(|t| path.starts_with(t)) {
                out.extend(functions);
            }
        }
    }
    out
}

/// The packages of the workspace whose root is `root`, as cargo decides
/// them and read apart from the engine (which asks `cargo metadata`): the
/// root itself when its manifest has a `[package]`, and every directory a
/// `workspace.members` entry names or matches, less `workspace.exclude`. A
/// package below `root` that is not a member is not this workspace's.
fn members(root: &Path) -> Vec<PathBuf> {
    let manifest: toml::Value = toml::from_str(
        &std::fs::read_to_string(root.join("Cargo.toml")).expect("the root manifest"),
    )
    .expect("the root manifest parses");
    let list = |key: &str| -> Vec<String> {
        manifest
            .get("workspace")
            .and_then(|w| w.get(key))
            .and_then(|v| v.as_array())
            .into_iter()
            .flatten()
            .map(|v| v.as_str().expect("a path").to_string())
            .collect()
    };
    let excluded: Vec<PathBuf> = list("exclude").iter().map(|e| root.join(e)).collect();
    let mut out = Vec::new();
    if manifest.get("package").is_some() {
        out.push(root.to_path_buf());
    }
    for pattern in list("members") {
        let mut found = vec![root.to_path_buf()];
        for part in pattern.split('/').filter(|p| !p.is_empty() && *p != ".") {
            assert!(
                !part.contains("**") && !part.contains('['),
                "{pattern}: a member glob this reader does not expand"
            );
            let mut next = Vec::new();
            for dir in found {
                if !part.contains(['*', '?']) {
                    next.push(dir.join(part));
                    continue;
                }
                let mut matched: Vec<PathBuf> = std::fs::read_dir(&dir)
                    .expect("a member's parent")
                    .map(|e| e.expect("entry").path())
                    .filter(|p| p.is_dir())
                    .filter(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        glob_matches(part, name)
                    })
                    .collect();
                matched.sort();
                next.extend(matched);
            }
            found = next;
        }
        for dir in found {
            assert!(
                dir.join("Cargo.toml").is_file(),
                "{pattern}: {} is a member with no manifest",
                dir.display()
            );
            if !excluded.contains(&dir) && !out.contains(&dir) {
                out.push(dir);
            }
        }
    }
    out
}

/// Whether one path component `name` matches the glob component `pattern`
/// (`*` any run of characters, `?` one character), as cargo's member globs.
fn glob_matches(pattern: &str, name: &str) -> bool {
    fn at(p: &[char], n: &[char]) -> bool {
        match p.split_first() {
            None => n.is_empty(),
            Some(('*', rest)) => (0..=n.len()).any(|i| at(rest, &n[i..])),
            Some(('?', rest)) => !n.is_empty() && at(rest, &n[1..]),
            Some((c, rest)) => n.first() == Some(c) && at(rest, &n[1..]),
        }
    }
    let (p, n): (Vec<char>, Vec<char>) = (pattern.chars().collect(), name.chars().collect());
    at(&p, &n)
}

/// Product code is what cargo makes a member of the workspace: the walker
/// reads the root manifest's `workspace.members` and `exclude` itself, so a
/// member with no row fails this test whatever its directory is called, and a
/// package that is not a member is not counted.
///
/// Left outside by that rule today, as an open row for a later projection of
/// that workspace under its own profile: the three packages the installed
/// Prism factory brings, none a member here. `prism-factory/host` declares
/// its own `[workspace]` and is upstream's factory host, held to upstream's
/// text by `prism-factory/upstream.toml` and `graph.py`; its one adapted file
/// is covered by the factory's own gate (UE-20), not by this lift register.
/// `prism-factory/certified/packages/rust-std/factory_plan` and
/// `factory_reweigh` are Rust that a LexLean production generated and that
/// `verify-artifact` replays byte for byte.
#[test]
fn every_function_of_product_code_has_exactly_one_row_ue_16() {
    // The walker's own rule, on a planted workspace: members by path and by
    // glob count, an excluded directory, a package that is not a member (its
    // own `[workspace]`, beside the members) and an out-of-line test module
    // do not.
    let package = |name: &str| format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\n");
    let planted = scratch(
        "ue16-members",
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"crates/*\", \"tool\"]\nexclude = [\"crates/skipped\"]\n",
            ),
            ("crates/alpha/Cargo.toml", &package("alpha")),
            (
                "crates/alpha/src/lib.rs",
                "/// A.\npub fn alpha() {}\n/// T.\npub struct T;\nimpl T {\n    /// M.\n    pub fn method(&self) {}\n}\n",
            ),
            ("crates/beta/Cargo.toml", &package("beta")),
            (
                "crates/beta/src/lib.rs",
                "/// B.\npub fn beta() {}\nmod inner;\n#[cfg(test)]\nmod tests;\n",
            ),
            ("crates/beta/src/inner.rs", "/// I.\npub fn inner() {}\n"),
            ("crates/beta/src/tests.rs", "/// Only a test's.\npub fn helper() {}\n"),
            ("crates/beta/src/tests/deep.rs", "/// A test's too.\npub fn deep() {}\n"),
            ("crates/skipped/Cargo.toml", &package("skipped")),
            ("crates/skipped/src/lib.rs", "/// S.\npub fn skipped() {}\n"),
            ("tool/Cargo.toml", &package("tool")),
            ("tool/src/main.rs", "fn main() {}\n"),
            (
                "outside/host/Cargo.toml",
                &format!("{}\n[workspace]\n", package("host")),
            ),
            ("outside/host/src/lib.rs", "/// H.\npub fn host() {}\n"),
        ],
    );
    let names: Vec<String> = product_functions(&planted)
        .iter()
        .map(|f| f.rsplit(": ").next().unwrap_or_default().to_string())
        .collect();
    assert_eq!(
        names,
        ["alpha", "impl method", "inner", "beta", "main"],
        "{names:?}"
    );
    let _ = std::fs::remove_dir_all(planted);

    for (root, name) in [(root(), "uorust"), (fixture(), "thermostat")] {
        let files = project(&root, name);
        let frontier: toml::Value =
            toml::from_str(&text(&files, "frontier.toml")).expect("frontier parses");
        let mut rows: BTreeMap<String, Vec<&str>> = BTreeMap::new();
        for table in ["residue", "foreign", "effect", "lifted"] {
            for row in frontier
                .get(table)
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
            {
                let function = row
                    .get("function")
                    .and_then(|v| v.as_str())
                    .expect("a function")
                    .to_string();
                rows.entry(function).or_default().push(table);
            }
        }
        for (function, tables) in &rows {
            assert_eq!(tables.len(), 1, "{name}: {function} has rows in {tables:?}");
        }
        let source = product_functions(&root);
        assert_eq!(
            rows.len(),
            source.len(),
            "{name}: {} functions of product code, {} with a row",
            source.len(),
            rows.len()
        );
    }
}

/// The planted defect: a function the incumbent register records as lifted,
/// whose own source is unchanged (so its κ is the same question), is decided
/// unlifted because a function it calls stopped lifting. The gate refuses the
/// projection, by the register and by `uorust::write` into the old output,
/// and leaves the old projection in place.
#[test]
fn a_lifted_kappa_that_turns_unlifted_is_refused_ue_16() {
    use uorust::register::{admit, Status};
    let manifest =
        "[package]\nname = \"ratchet\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[workspace]\n";
    let caller = "/// Calls the test.\npub fn ready(x: u8) -> bool {\n    over(x)\n}\n";
    let before = scratch(
        "ue16-before",
        &[
            ("Cargo.toml", manifest),
            (
                "src/lib.rs",
                &format!("//! R.\n\n/// The test.\npub fn over(x: u8) -> bool {{\n    x > 1\n}}\n\n{caller}"),
            ),
        ],
    );
    let after = scratch(
        "ue16-after",
        &[
            ("Cargo.toml", manifest),
            (
                "src/lib.rs",
                &format!("//! R.\n\n/// The test.\npub fn over(x: u8) -> bool {{\n    x + 1 > 2\n}}\n\n{caller}"),
            ),
        ],
    );
    let old = project(&before, "ratchet");
    let new = project(&after, "ratchet");
    let incumbent = uorust::lift_register(&old).expect("a register");
    let candidate = uorust::lift_register(&new).expect("a register");
    let (kappa, row) = incumbent
        .rows
        .iter()
        .find(|(_, r)| r.function == "ratchet::ready")
        .expect("ready has a row");
    assert!(matches!(row.status, Status::Lifted(_)), "{row:?}");
    // The same question, now decided otherwise.
    assert!(
        !matches!(candidate.rows[kappa].status, Status::Lifted(_)),
        "{:?}",
        candidate.rows[kappa]
    );
    let refused = admit(&incumbent, &candidate);
    assert!(
        refused.len() == 1 && refused[0].contains(kappa.as_str()),
        "{refused:?}"
    );
    // The changed function is a new question: no regression of its own.
    assert!(refused.iter().all(|r| !r.starts_with("ratchet::over")));
    // Projecting into the incumbent's output is refused, and writes nothing.
    let out = std::env::temp_dir().join(format!("uorust-ue16-out-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&out);
    uorust::write(&out, &old).expect("a first projection");
    match uorust::write(&out, &new) {
        Err(UorustError::Regressed(rows)) => assert_eq!(rows, refused),
        other => panic!("expected Regressed, got {other:?}"),
    }
    assert!(matches!(
        uorust::check(&out, &new),
        Err(UorustError::Regressed(_))
    ));
    assert_eq!(
        std::fs::read(out.join("frontier.toml")).expect("kept"),
        old["frontier.toml"]
    );
    for dir in [before, after, out] {
        let _ = std::fs::remove_dir_all(dir);
    }
}

fn mir_fixture() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mir-path")
}

/// The fixture projected through the MIR path on `verdicts`.
fn walk(
    verdicts: uorust::mir::verdict::Verdicts,
    trial: uorust::mir::Trial,
) -> BTreeMap<String, Vec<u8>> {
    let root = mir_fixture();
    let mut options = options("walk");
    options.mir = Some(mir_options(dumped(&root, "walk"), verdicts, trial));
    uorust::project_workspace(&root, &options).expect("projects")
}

/// The detail of the residue row of `function` in a projection's frontier.
fn residue_detail(files: &BTreeMap<String, Vec<u8>>, function: &str) -> Option<String> {
    let frontier: toml::Value = toml::from_str(&text(files, "frontier.toml")).expect("frontier");
    frontier
        .get("residue")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .find(|r| r.get("function").and_then(|v| v.as_str()) == Some(function))
        .and_then(|r| r.get("detail").and_then(|v| v.as_str()))
        .map(str::to_string)
}

fn lifted_functions(files: &BTreeMap<String, Vec<u8>>) -> Vec<String> {
    let frontier: toml::Value = toml::from_str(&text(files, "frontier.toml")).expect("frontier");
    frontier
        .get("lifted")
        .and_then(|v| v.as_array())
        .into_iter()
        .flatten()
        .filter_map(|r| r.get("function").and_then(|v| v.as_str()))
        .map(str::to_string)
        .collect()
}

#[test]
fn every_mir_kind_of_the_pinned_toolchain_has_a_coverage_row_ue_17() {
    let toolchain = std::fs::read_to_string(root().join("rust-toolchain.toml")).expect("toolchain");
    let source =
        std::fs::read_to_string(root().join("crates/uorust/mir/SOURCE.toml")).expect("source");
    let channel = |text: &str, key: &str| {
        text.lines()
            .find_map(|l| l.strip_prefix(&format!("{key} = \"")))
            .and_then(|v| v.strip_suffix('"'))
            .unwrap_or_default()
            .to_string()
    };
    assert_eq!(
        channel(&source, "toolchain"),
        channel(&toolchain, "channel")
    );
    let report = uorust::mir::coverage::report();
    assert!(report.contains(&format!("at toolchain {}", channel(&toolchain, "channel"))));
    for kind in [
        "StatementKind::Assign: lowered",
        "TerminatorKind::SwitchInt: lowered",
        "TerminatorKind::Assert: lowered",
        "Rvalue::BinaryOp: lowered",
        "BinOp::AddWithOverflow: lowered",
        "Rvalue::RawPtr: residue",
        "TerminatorKind::InlineAsm: residue",
    ] {
        assert!(report.contains(kind), "no coverage row {kind}");
    }
}

#[test]
fn the_mir_path_translates_and_admits_on_verdicts_ue_17() {
    use uorust::mir::verdict::Decision;
    use uorust::mir::Trial;
    // The dump: one file per body of the library.
    let fresh = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mir-walk-fresh");
    uorust::mir::dump(&mir_fixture(), &fresh).expect("the MIR dumps");
    assert!(std::fs::read_dir(fresh.join("lib-walk"))
        .expect("the library's dumps")
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .any(|n| n.starts_with("walk.count.")));
    // The trial run: every translation is admitted, with its loop helper,
    // the helper's measure and decrease theorems, and a theorem per
    // obligation; the prover reads the listing of candidates.
    let trial = walk(Default::default(), Trial::All);
    let behavior = text(&trial, "src/Behavior.lex.tex");
    for decl in [
        "name: walkCappedSumLoop1,",
        "name: decreases_walkCappedSumLoop1_0,",
        "name: holds_walkCappedSum_0,",
        "name: walkCappedSum,",
        "name: walkCount,",
        "termination: {",
        // std's own `Option::unwrap_or`, lifted from its vendored MIR at
        // `u32`, and a promoted constant lowered from its own body.
        "name: stdOptionOptionUnwrapOrU32,",
        "name: walkAbove,",
        // `BTreeSet::insert` by its model, a backward function of the
        // mutable borrow; `str::trim` by its model over the prelude.
        "name: stdInsertString,",
        "(stdInsertString (stdInsertString (nil String) a) b)",
        "(Prelude::textTrim ",
        // A `for` loop over a slice: its iterator a cursor, the loop's
        // measure the distance between the cursor's positions, which its
        // guarded `next` is `Some` within.
        "name: walkFirstRiseLoop2,",
        "measure: (prim subtract (project iter item2) (project iter item1) : Nat)",
        "name: stdAtNat,",
        // A closure: a definition over the value of its captures, called
        // from std's `find`, which is lifted from std's own MIR.
        "name: closureFirstNamed0,",
        "name: stdSliceIterIterFindClosure",
    ] {
        assert!(behavior.contains(decl), "the trial projection lacks {decl}");
    }
    let listing = text(&trial, uorust::mir::CANDIDATES);
    for function in ["walk::capped_sum", "walk::count"] {
        assert!(
            listing.contains(&format!("function = \"{function}\"\ndigest = ")),
            "{function} is not a candidate"
        );
    }
    // A kind or type the translator does not lower is a residue row that
    // names it.
    assert_eq!(
        residue_detail(&trial, "walk::bump").as_deref(),
        Some("mir::Ty: std::cell::Cell<u32>")
    );

    // The committed verdicts: the proved translation lifts, the refused one
    // is residue naming the verifier's diagnostic.
    let committed = verdicts(&mir_fixture().join("lift-verdicts.toml"));
    let decided = walk(committed.clone(), Trial::Off);
    for f in [
        "walk::capped_sum",
        "walk::distinct",
        "walk::first_blank",
        "walk::above",
        // `for` loops over a slice and a set, `break`, and `?` on an option.
        "walk::first_rise",
        "walk::sum_until",
        "walk::second_known",
        // A closure passed to std's `find`.
        "walk::first_named",
        // Signed integers compared as their bit patterns.
        "walk::first_negative",
    ] {
        assert!(
            lifted_functions(&decided).contains(&f.to_string()),
            "{f} does not lift"
        );
    }
    assert!(text(&decided, "src/Behavior.lex.tex").contains("name: walkCappedSumLoop1,"));
    assert!(!decided.contains_key(uorust::mir::CANDIDATES));
    let count = residue_detail(&decided, "walk::count").expect("count is residue");
    assert!(count.starts_with("mir::Verdict: LLV"), "{count}");
    // A loop helper whose fixed parameter only a closure's capture uses:
    // Lean's `decreasing_by` context drops it, so the verifier refuses.
    let any = residue_detail(&decided, "walk::any_above").expect("any_above is residue");
    assert!(any.starts_with("mir::Verdict: LLV"), "{any}");
    // The MIR path only adds: whatever lifts without it lifts with it.
    let surface = project(&mir_fixture(), "walk");
    for f in lifted_functions(&surface) {
        assert!(
            lifted_functions(&decided).contains(&f),
            "{f} stopped lifting"
        );
    }

    // A verdict of another digest admits nothing: a changed translation is a
    // new question.
    let mut stale = committed;
    for v in stale.rows.values_mut() {
        if v.status == Decision::Proved {
            v.digest = "0".repeat(64);
        }
    }
    let planted = walk(stale, Trial::Off);
    assert!(!lifted_functions(&planted).contains(&"walk::capped_sum".to_string()));
    assert_eq!(
        residue_detail(&planted, "walk::capped_sum").as_deref(),
        Some("mir::Verdict: a translation the verifier has not decided yet")
    );
    assert!(!text(&planted, "src/Behavior.lex.tex").contains("name: walkCappedSum,"));
}

/// `prove` on a register that decides every translation runs no verifier and
/// writes the same register back.
#[test]
fn the_prover_keeps_a_decided_register_ue_17() {
    let root = mir_fixture();
    let committed = std::fs::read_to_string(root.join("lift-verdicts.toml")).expect("verdicts");
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join("mir-walk-prove");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch");
    let verdicts = dir.join("lift-verdicts.toml");
    std::fs::write(&verdicts, &committed).expect("verdicts");
    let summary = uorust::mir::prove(
        &root,
        &options("walk"),
        &dumped(&root, "walk"),
        &verdicts,
        &dir.join("scratch"),
    )
    .expect("proves");
    assert_eq!((summary.proved, summary.refused, summary.runs), (0, 0, 0));
    assert_eq!(
        std::fs::read_to_string(&verdicts).expect("verdicts"),
        committed
    );
}

/// A planted defect: a shard whose rust-src is absent. Its census refuses,
/// so the shard and the run are red, rather than translating every body
/// against no declarations and recording what it got.
#[test]
fn a_census_without_its_library_sources_refuses() {
    use uorust::mir::core;
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core");
    let missing = fixture.join("absent/library");
    assert!(!missing.exists());
    assert!(
        core::core_census(&missing, &fixture.join("mir")).is_err(),
        "a census with no rust-src must refuse"
    );
}

/// Each library workflow's shards read rust-src before translating, and
/// its merge writes the register only when every shard succeeded.
#[test]
fn library_shards_read_their_sources_and_merge_only_whole() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    for workflow in ["core.yml", "std.yml"] {
        let text = std::fs::read_to_string(root.join(".github/workflows").join(workflow))
            .expect("workflow");
        let translate = text
            .lines()
            .find(|l| l.contains("--only '$MODULE'"))
            .expect("the shard's census");
        let fetch = translate
            .find("scripts/mir/dump-lib.sh")
            .expect("rust-src fetched");
        assert!(fetch < translate.find("target/bin/uorust").expect("census"));
        let merge = &text[text.find("\n  merge:").expect("merge job")..];
        let refuse = merge
            .find("Refuse a partial set of shards")
            .expect("refusal");
        assert!(merge[refuse..].contains("needs.shard.result != 'success'"));
        assert!(refuse < merge.find("uorust merge").expect("merge step"));
        // The register lands through land-register.sh, which pushes it
        // with push.sh: both after the refusal.
        let land = merge
            .find("scripts/ci/land-register.sh")
            .expect("landing step");
        assert!(refuse < land);
        let script = std::fs::read_to_string(root.join("scripts/ci/land-register.sh"))
            .expect("land-register.sh");
        assert!(script.contains("scripts/ci/push.sh"));
    }
}

/// The witness of `crates/uorust/core/retracted.toml`'s row: a switch on
/// `cmp::Ordering`'s discriminant reads the discriminants core declares
/// (-1, 0, 1; a switch prints -1 as its `i8` bits, 255), not the variants'
/// indices. Read by index, the arm on 255 matched no variant and its call
/// was never lowered, so the body translated as if `Less` took the
/// otherwise arm (and `then_with`'s arm on 0 ran its closure on `Less`).
#[test]
fn a_switch_on_ordering_reads_its_declared_discriminants() {
    use uorust::mir::core;
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/ordering");
    let shards = core::core_census(&fixture.join("library"), &fixture.join("mir")).expect("census");
    let row = shards
        .iter()
        .flat_map(|s| &s.rows)
        .find(|r| r.path == "core::cmp::less_reaches")
        .expect("less_reaches");
    // `Less` reaches the call this fixture leaves undefined.
    assert!(row.digest.is_none(), "{row:?}");
    let cause = row.cause.as_deref().unwrap_or_default();
    assert!(cause.contains("absent"), "{cause}");
    let row = |path: &str| {
        shards
            .iter()
            .flat_map(|s| &s.rows)
            .find(|r| r.path == path)
            .unwrap_or_else(|| panic!("{path}"))
    };
    // An arm on each declared discriminant leaves the otherwise arm
    // unreached; read by index, `Greater` (2) fell to it.
    let every = row("core::cmp::every_arm");
    assert!(every.digest.is_some(), "{every:?}");
    // `Less`, the first variant, takes the otherwise arm, not the arm on 0.
    let equal = row("core::cmp::equal_arm");
    let cause = equal.cause.as_deref().unwrap_or_default();
    assert!(cause.contains("on_other"), "{cause}");
}

/// The witness of `crates/uorust/core/retracted.toml`'s `int_sqrt` rows:
/// rustc prints two constant items of `u32_stages`, `middle_stage!`'s
/// `HALF_BITS` (`u16::BITS >> 1`) and `last_stage!`'s (`u32::BITS >> 1`),
/// and every use of either, under one path; only the dumps' file names
/// tell them apart (`HALF_BITS#1`). The census took the first dump it read
/// for the path, `last_stage!`'s, so `middle_stage!`'s `u16`
/// `LOWER_HALF_1_BITS` read 16 where its own item is 8. A use of such a
/// path is now residue.
#[test]
fn a_constant_path_rustc_prints_for_two_items_is_refused() {
    use uorust::mir::core;
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/twins");
    // The two items have other values: `u32`'s `BITS` and `u16`'s.
    let mut names: Vec<PathBuf> = std::fs::read_dir(fixture.join("mir/num"))
        .expect("dumps")
        .map(|e| e.expect("dump").path())
        .filter(|p| p.to_string_lossy().contains("-HALF_BITS"))
        .collect();
    names.sort();
    let texts: Vec<String> = names
        .iter()
        .map(|p| std::fs::read_to_string(p).expect("dump"))
        .collect();
    assert_eq!(texts.len(), 2);
    // The first read, the one the old census took, is `u32`'s; the `u16`
    // constant that uses it is `middle_stage!`'s, whose own is `u16`'s.
    assert!(
        texts[0].contains("const num::<impl u32>::BITS"),
        "{}",
        texts[0]
    );
    assert!(
        texts[1].contains("const num::<impl u16>::BITS"),
        "{}",
        texts[1]
    );
    let lower = std::fs::read_to_string(
        fixture.join("mir/num/core.num-imp-int_sqrt-u32_stages-LOWER_HALF_1_BITS.2-2-012.ElaborateBoxDerefs.before.mir"),
    )
    .expect("lower");
    assert!(lower.contains("const u32_stages::LOWER_HALF_1_BITS: u16"));
    let shards = core::core_census(&fixture.join("library"), &fixture.join("mir")).expect("census");
    let rows: Vec<&core::CoreRow> = shards.iter().flat_map(|s| &s.rows).collect();
    let half: Vec<&&core::CoreRow> = rows
        .iter()
        .filter(|r| r.path.contains("::HALF_BITS"))
        .collect();
    assert_eq!(half.len(), 2, "{rows:?}");
    assert!(half.iter().all(|r| r.digest.is_some()), "{half:?}");
    assert_ne!(
        half[0].digest, half[1].digest,
        "the two items' values differ"
    );
    let user = rows
        .iter()
        .find(|r| r.path.ends_with("::LOWER_HALF_1_BITS"))
        .expect("LOWER_HALF_1_BITS");
    let cause = user.cause.as_deref().unwrap_or_default();
    assert!(cause.contains("more than one constant item"), "{user:?}");
}

/// Every retraction of a library register names its body by a κ and path
/// the register holds, and its witness is a test of these tests, which
/// `cargo test` runs: a retraction whose witness fails is a red run.
#[test]
fn every_retraction_is_witnessed() {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut tests = String::new();
    for entry in std::fs::read_dir(crate_dir.join("tests")).expect("tests") {
        let p = entry.expect("entry").path();
        if p.extension().is_some_and(|x| x == "rs") {
            tests.push_str(&std::fs::read_to_string(&p).expect("test source"));
        }
    }
    for register in ["core", "std"] {
        let dir = crate_dir.join(register);
        for r in uorust::queue::retractions(&dir).expect("retractions") {
            let test = format!("#[test]\nfn {}() {{", r.witness);
            assert!(tests.contains(&test), "{}: no test {}", r.path, r.witness);
            let held = std::fs::read_dir(&dir)
                .expect("register")
                .filter_map(Result::ok)
                .filter(|e| {
                    e.path()
                        .file_stem()
                        .is_some_and(|n| n != uorust::queue::RETRACTED)
                })
                .filter_map(|e| std::fs::read_to_string(e.path()).ok())
                .any(|t| t.contains(&format!("path = {:?}\nkappa = \"{}\"", r.path, r.kappa)));
            assert!(held, "{} is no row of the {register} register", r.path);
        }
    }
}

/// Core's register: each body of a module shard translated at the canonical
/// instantiation and recorded by κ; a body recorded as translated that is
/// residue later is a regression the register refuses.
#[test]
fn core_bodies_are_translated_into_a_monotone_register() {
    use uorust::mir::core;
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core");
    let shards = core::core_census(&fixture.join("library"), &fixture.join("mir")).expect("census");
    assert_eq!(shards.len(), 1);
    let bool_shard = &shards[0];
    assert_eq!(bool_shard.module, "bool");
    let status = |path: &str| {
        bool_shard
            .rows
            .iter()
            .find(|r| r.path == path)
            .map(|r| r.digest.is_some())
    };
    // The impl is named by the type its source implements; a generic body
    // is translated with its parameters at the canonical type.
    assert_eq!(status("core::bool::bool::then_some"), Some(true));
    assert_eq!(status("core::bool::bool::ok_or"), Some(true));
    // `then` calls its closure parameter, which the canonical type is not.
    assert_eq!(status("core::bool::bool::then"), Some(false));
    let row = bool_shard
        .rows
        .iter()
        .find(|r| r.path == "core::bool::bool::then_some")
        .expect("then_some");
    assert_eq!(row.at, format!("T={}", core::CANONICAL));

    let dir = std::env::temp_dir().join(format!("uorust-core-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    core::write_core_register(&dir, &shards).expect("writes");
    let summary = std::fs::read_to_string(dir.join("frontier.toml")).expect("summary");
    assert!(summary.contains("[module.bool]\nbodies = 4\ntranslated = 2\nresidue = 2"));
    assert!(core::core_regressions(&dir, &shards).is_empty());
    // A register that records `then` as translated refuses the census that
    // leaves it residue.
    let rows = std::fs::read_to_string(dir.join("bool.toml")).expect("rows");
    let then = rows
        .split("\n\n")
        .find(|r| r.contains("path = \"core::bool::bool::then\""))
        .expect("then's row");
    let forged = then
        .lines()
        .take_while(|l| !l.starts_with("status"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\nstatus = \"translated\"\ndigest = \"0\"";
    std::fs::write(dir.join("bool.toml"), rows.replace(then, &forged)).expect("forges");
    let regressed = core::core_regressions(&dir, &shards);
    assert_eq!(regressed.len(), 1, "{regressed:?}");
    assert!(regressed[0].starts_with("core::bool::bool::then:"));
    // Retracted by its κ and path, it may be residue; under another path,
    // or with no witness, it still refuses.
    let kappa = then
        .lines()
        .find_map(|l| l.strip_prefix("kappa = "))
        .expect("then's kappa");
    let retract = |path: &str, witness: &str| {
        std::fs::write(
            dir.join("retracted.toml"),
            format!("[[retracted]]\npath = {path:?}\nkappa = {kappa}\nreason = \"r\"\nwitness = {witness:?}\n"),
        )
        .expect("retracts");
        core::core_regressions(&dir, &shards)
    };
    assert!(retract("core::bool::bool::then", "w").is_empty());
    assert_eq!(retract("core::bool::bool::then_some", "w").len(), 1);
    assert_eq!(retract("core::bool::bool::then", "").len(), 1);
    let _ = std::fs::remove_dir_all(&dir);
}

/// Every body of the core fixture is embedded as a value of the MIR
/// datatype beside the semantics, with its census; dumps that do not say
/// the target they were built for are refused, since an integer's width
/// depends on it.
#[test]
fn every_core_body_is_embedded_ue_19() {
    let fixture = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/core");
    let work = std::env::temp_dir().join(format!("uorust-embed-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&work);
    let out = work.join("out");
    let refused = uorust::embed::embed(
        "core",
        &fixture.join("library"),
        &fixture.join("mir"),
        &out,
        &[],
        &uorust::embed::Outputs::default(),
    );
    assert!(
        matches!(refused, Err(UorustError::Malformed { .. })),
        "{refused:?}"
    );
    // The fixture's dumps with the target the bodies were built for.
    let dumps = work.join("mir");
    std::fs::create_dir_all(dumps.join("bool")).expect("the dumps");
    for entry in std::fs::read_dir(fixture.join("mir/bool")).expect("the fixture") {
        let entry = entry.expect("a dump");
        std::fs::copy(entry.path(), dumps.join("bool").join(entry.file_name())).expect("copied");
    }
    std::fs::write(
        dumps.join("target.toml"),
        "triple = \"x86_64-unknown-linux-gnu\"\npointer_width = 64\n",
    )
    .expect("the target");
    let harness = work.join("harness");
    let census = uorust::embed::embed(
        "core",
        &fixture.join("library"),
        &dumps,
        &out,
        &[],
        &uorust::embed::Outputs {
            harness: Some(&harness),
            ..Default::default()
        },
    )
    .expect("the embedding");
    assert_eq!(census.bodies, 4);
    assert!(census.evaluable <= census.bodies);
    let rendered = census.render();
    assert!(rendered.contains("bodies = 4"), "{rendered}");
    let part =
        std::fs::read_to_string(out.join("src/CoreMir/BoolPart0.lex.tex")).expect("the bodies");
    assert_eq!(part.matches("result: Mir::Body").count(), 4);
    assert!(part.contains("\\importmodule{Mir}"));
    assert!(out.join("src/Mir.lex.tex").exists());
    let config = std::fs::read_to_string(out.join("lexlean.toml")).expect("the project");
    assert!(config.contains("\"src/CoreMir/BoolPart0.lex.tex\", \"src/Mir.lex.tex\""));
    assert_eq!(
        std::fs::read_to_string(out.join("census.toml")).expect("the census"),
        rendered
    );
    // No body of `bool` is a scalar method of an integer, so there is
    // nothing for a harness to call.
    assert!(!harness.exists());
    let _ = std::fs::remove_dir_all(&work);
}

#[test]
fn a_method_writing_its_struct_threads_it_ue_21() {
    let planted = scratch(
        "ue21-threading",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"mint\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "/// A mint.\npub struct Mint {\n    /// Its supply.\n    pub supply: u64,\n    \
                 /// Whether it is initialized.\n    pub is_initialized: u8,\n}\n\nimpl Mint {\n    \
                 /// Marks it initialized.\n    pub fn set_initialized(&mut self) {\n        \
                 self.is_initialized = 1;\n    }\n\n    /// Writes the supply twice.\n    \
                 pub fn reset(&mut self, supply: u64) {\n        self.supply = supply;\n        \
                 self.is_initialized = 0;\n        self.supply = 7;\n    }\n\n    \
                 /// Reads through a mutable borrow.\n    pub fn take(&mut self) -> u64 {\n        \
                 self.supply\n    }\n}\n",
            ),
        ],
    );
    let files = project(&planted, "mint");
    let lifted = lifted_functions(&files);
    for setter in ["mint::Mint::set_initialized", "mint::Mint::reset"] {
        assert!(
            lifted.iter().any(|f| f == setter),
            "{setter} lifts: {lifted:?}"
        );
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    // The record the model's threadedRecord gives: the written field holds
    // the value, every other is projected from the value before the write.
    assert!(
        behavior
            .contains("(record Mint {supply := (project selfValue supply), isInitialized := 1})"),
        "{behavior}"
    );
    // The last write to supply wins: 7 is bound after the parameter.
    assert!(behavior.contains("supply := 7"), "{behavior}");
    let detail = residue_detail(&files, "mint::Mint::take").expect("take is a residue row");
    assert!(detail.contains("takes &mut self of"), "{detail}");
}

#[test]
fn a_write_through_a_nested_place_threads_it_ue_22() {
    let planted = scratch(
        "ue22-places",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"places\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "/// An optional value: a tag and the value.\npub type COption<T> = ([u8; 4], T);\n\n\
                 /// An account.\npub struct Account {\n    /// Its mint.\n    pub mint: [u8; 32],\n    \
                 /// Its delegate.\n    pub delegate: COption<[u8; 32]>,\n    \
                 /// Its flags.\n    pub flags: [u8; 4],\n}\n\nimpl Account {\n    \
                 /// Clears the delegate.\n    pub fn clear_delegate(&mut self) {\n        \
                 self.delegate.0[0] = 0;\n    }\n\n    /// Sets the delegate.\n    \
                 pub fn set_delegate(&mut self, delegate: &[u8; 32]) {\n        \
                 self.delegate.0[0] = 1;\n        self.delegate.1 = *delegate;\n    }\n\n    \
                 /// Sets a flag it computes.\n    pub fn set_flag(&mut self, at: usize) {\n        \
                 self.flags[at] = 1;\n    }\n}\n",
            ),
        ],
    );
    let files = project(&planted, "places");
    let lifted = lifted_functions(&files);
    for setter in [
        "places::Account::clear_delegate",
        "places::Account::set_delegate",
    ] {
        assert!(
            lifted.iter().any(|f| f == setter),
            "{setter} lifts: {lifted:?}"
        );
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    // The index write keeps the tail and writes the head; the item write
    // projects the other item from the tuple before the write.
    assert!(behavior.contains("=> (cons 0 "), "{behavior}");
    assert!(behavior.contains("=> (cons 1 "), "{behavior}");
    assert!(behavior.contains("item1 := delegate"), "{behavior}");
    let detail = residue_detail(&files, "places::Account::set_flag").expect("set_flag is residue");
    assert!(detail.contains("place"), "{detail}");
}

#[test]
fn bytes_flags_and_unit_enums_convert_by_the_prelude_ue_23() {
    let planted = scratch(
        "ue23-conversions",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"bytes\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "//! Bytes.\n/// A state.\n#[repr(u8)]\npub enum State {\n    /// Not yet.\n    Uninitialized,\n    \
                 /// Ready.\n    Initialized,\n    /// Stopped.\n    Frozen,\n}\n\n\
                 /// A code whose source sets its values.\npub enum Code {\n    /// Five.\n    A = 5,\n    /// Eight, as an expression.\n    B = 1 << 3,\n}\n\n\
                 /// A mint.\npub struct Mint {\n    /// Its supply.\n    pub supply: [u8; 8],\n    \
                 /// Its state.\n    pub state: u8,\n    /// Whether it is native.\n    pub native: u8,\n}\n\n\
                 impl Mint {\n    /// The supply.\n    pub fn supply(&self) -> u64 {\n        u64::from_le_bytes(self.supply)\n    }\n\n    \
                 /// Sets the supply.\n    pub fn set_supply(&mut self, supply: u64) {\n        self.supply = supply.to_le_bytes();\n    }\n\n    \
                 /// Sets the state.\n    pub fn set_state(&mut self, state: State) {\n        self.state = state as u8;\n    }\n\n    \
                 /// Sets the flag.\n    pub fn set_native(&mut self, value: bool) {\n        self.native = value as u8;\n    }\n}\n\n\
                 /// A code's value.\npub fn code(c: Code) -> u8 {\n    c as u8\n}\n",
            ),
        ],
    );
    let files = project(&planted, "bytes");
    let lifted = lifted_functions(&files);
    for f in [
        "bytes::Mint::supply",
        "bytes::Mint::set_supply",
        "bytes::Mint::set_state",
        "bytes::Mint::set_native",
    ] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    for needle in [
        "(Prelude::fromLeBytes (project selfValue supply))",
        "(Prelude::leBytes 8 supply)",
        "(Prelude::bitOf value)",
        "| State.Frozen => 2",
    ] {
        assert!(behavior.contains(needle), "{needle}: {behavior}");
    }
    let detail = residue_detail(&files, "bytes::code").expect("code is a residue row");
    assert!(detail.contains("discriminant"), "{detail}");
}

#[test]
fn a_crate_reads_the_types_of_the_crates_it_depends_on_ue_24() {
    let package = |name: &str, deps: &str| {
        format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\n{deps}")
    };
    let planted = scratch(
        "ue24-crates",
        &[
            (
                "Cargo.toml",
                "[workspace]\nmembers = [\"iface\", \"prog\", \"other\"]\nresolver = \"2\"\n",
            ),
            ("iface/Cargo.toml", &package("iface", "")),
            ("iface/src/lib.rs", "//! Iface.\n/// Errors.\npub mod error;\n"),
            (
                "iface/src/error.rs",
                "//! Errors.\n/// An error.\n#[derive(Clone, Debug, Eq, PartialEq)]\npub enum TokenError {\n    \
                 /// Bad.\n    InvalidInstruction,\n    /// Worse.\n    Other,\n}\n",
            ),
            ("prog/Cargo.toml", &package("prog", "iface = { path = \"../iface\" }\n")),
            (
                "prog/src/lib.rs",
                "//! Prog.\nuse {\n    iface::{\n        error::TokenError,\n    },\n};\n\n\
                 /// Through the import.\npub fn grouped(x: u64) -> Result<u64, TokenError> {\n    \
                 if x > 0 {\n        Ok(x)\n    } else {\n        Err(TokenError::InvalidInstruction)\n    }\n}\n\n\
                 /// Through the full path.\npub fn pathed(x: u64) -> Result<u64, iface::error::TokenError> {\n    \
                 if x > 0 {\n        Ok(x)\n    } else {\n        Err(iface::error::TokenError::Other)\n    }\n}\n",
            ),
            ("other/Cargo.toml", &package("other", "")),
            (
                "other/src/lib.rs",
                "//! Other.\n/// A crate that does not depend on iface.\npub fn apart(x: u64) -> Result<u64, TokenError> {\n    \
                 Ok(x)\n}\n",
            ),
        ],
    );
    let files = project(&planted, "crates");
    let lifted = lifted_functions(&files);
    for f in ["prog::grouped", "prog::pathed"] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let detail = residue_detail(&files, "other::apart").expect("apart is a residue row");
    assert!(detail.contains("TokenError"), "{detail}");
}

#[test]
fn an_error_converts_by_its_from_implementation_ue_25() {
    let planted = scratch(
        "ue25-from",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"errors\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "//! Errors.\n/// A token error.\npub enum TokenError {\n    /// Bad.\n    InvalidInstruction,\n    /// Worse.\n    Other,\n}\n\n\
                 /// A program error.\npub enum ProgramError {\n    /// A custom code.\n    Custom(u32),\n    /// Bad input.\n    InvalidArgument,\n}\n\n\
                 impl From<TokenError> for ProgramError {\n    fn from(e: TokenError) -> Self {\n        ProgramError::Custom(e as u32)\n    }\n}\n\n\
                 /// Unpacks.\npub fn unpack(x: u64) -> Result<u64, TokenError> {\n    \
                 if x > 0 {\n        Ok(x)\n    } else {\n        Err(TokenError::InvalidInstruction)\n    }\n}\n\n\
                 /// Through `?`.\npub fn process(x: u64) -> Result<u64, ProgramError> {\n    let a = unpack(x)?;\n    Ok(a)\n}\n\n\
                 /// Through `into`.\npub fn reject() -> Result<u64, ProgramError> {\n    Err(TokenError::Other.into())\n}\n\n\
                 /// No conversion.\npub fn mismatch(x: u64) -> Result<u64, u32> {\n    let a = unpack(x)?;\n    Ok(a)\n}\n",
            ),
        ],
    );
    let files = project(&planted, "errors");
    let lifted = lifted_functions(&files);
    for f in ["errors::process", "errors::reject"] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    assert!(
        behavior
            .contains("(ctor Result.error [Nat, ProgramError] (programErrorFromTokenErrorFrom "),
        "{behavior}"
    );
    let detail = residue_detail(&files, "errors::mismatch").expect("mismatch is a residue row");
    assert!(detail.contains("? of another type"), "{detail}");
}

#[test]
fn self_and_its_associated_types_name_the_impl_ue_26() {
    let planted = scratch(
        "ue26-self",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"states\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "//! States.\n/// A state.\npub enum State {\n    /// Not yet.\n    Uninitialized,\n    /// Ready.\n    Initialized,\n}\n\n\
                 /// An error.\npub enum Error {\n    /// Bad.\n    Invalid,\n}\n\n\
                 impl TryFrom<u8> for State {\n    type Error = Error;\n\n    \
                 fn try_from(value: u8) -> Result<Self, Self::Error> {\n        match value {\n            \
                 0 => Ok(State::Uninitialized),\n            1 => Ok(State::Initialized),\n            _ => Err(Error::Invalid),\n        }\n    }\n}\n\n\
                 /// A holder.\npub struct Holder {\n    /// Its state byte.\n    pub state: u8,\n}\n\n\
                 impl Holder {\n    /// Its state.\n    pub fn state(&self) -> Result<State, Error> {\n        State::try_from(self.state)\n    }\n\n    \
                 /// Its state from a flag, which no impl takes.\n    pub fn flagged(&self, f: bool) -> Result<State, Error> {\n        State::try_from(f)\n    }\n}\n",
            ),
        ],
    );
    let files = project(&planted, "states");
    let lifted = lifted_functions(&files);
    for f in ["states::State::try_from", "states::Holder::state"] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let detail =
        residue_detail(&files, "states::Holder::flagged").expect("flagged is a residue row");
    assert!(detail.contains("try_from"), "{detail}");
}

#[test]
fn an_integer_read_as_a_fieldless_enum_is_its_variant_ue_27() {
    let planted = scratch(
        "ue27-transmute",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"codes\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
            ),
            (
                "src/lib.rs",
                "//! Codes.\n/// A code whose source sets its values.\n#[repr(u8)]\npub enum Code {\n    /// Five.\n    A = 5,\n    /// Nine.\n    B = 9,\n    /// Ten.\n    C,\n}\n\n\
                 /// The code of a byte.\npub fn code(v: u8) -> Option<Code> {\n    match v {\n        \
                 5 | 9..=10 => Some(unsafe { core::mem::transmute::<u8, Code>(v) }),\n        _ => None,\n    }\n}\n\n\
                 /// A byte no variant has.\npub fn wide(v: u8) -> Option<Code> {\n    match v {\n        \
                 5..=6 => Some(unsafe { core::mem::transmute::<u8, Code>(v) }),\n        _ => None,\n    }\n}\n\n\
                 /// A code's value.\npub fn value(c: Code) -> u8 {\n    c as u8\n}\n",
            ),
        ],
    );
    let files = project(&planted, "codes");
    let lifted = lifted_functions(&files);
    for f in ["codes::code", "codes::value"] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    for needle in ["| Code.B => 9", "| Code.C => 10"] {
        assert!(behavior.contains(needle), "{needle}: {behavior}");
    }
    let detail = residue_detail(&files, "codes::wide").expect("wide is a residue row");
    assert!(detail.contains("unsafe"), "{detail}");
}

#[test]
fn account_metas_and_branch_hints_lift_by_their_models_ue_28() {
    let planted = scratch(
        "ue28-builders",
        &[
            (
                "Cargo.toml",
                "[package]\nname = \"builder\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n\
                 [dependencies]\nsolana-instruction = \"2\"\nsolana-pubkey = \"2\"\npinocchio = \"0.9\"\n",
            ),
            (
                "src/lib.rs",
                "//! Builders.\nuse pinocchio::hint::{likely, unlikely};\nuse solana_instruction::AccountMeta;\nuse solana_pubkey::Pubkey;\n\n\
                 /// The metas of a writable source and a read-only signer.\n\
                 pub fn metas(source: Pubkey, owner: Pubkey) -> Vec<AccountMeta> {\n    \
                 let mut accounts = Vec::with_capacity(2);\n    \
                 accounts.push(AccountMeta::new(source, false));\n    \
                 accounts.push(AccountMeta::new_readonly(owner, true));\n    accounts\n}\n\n\
                 /// A hinted test.\npub fn hinted(a: bool, b: bool) -> bool {\n    likely(a) && !unlikely(b)\n}\n",
            ),
        ],
    );
    let files = project(&planted, "builder");
    let lifted = lifted_functions(&files);
    for f in ["builder::metas", "builder::hinted"] {
        assert!(lifted.iter().any(|l| l == f), "{f} lifts: {lifted:?}");
    }
    let behavior = text(&files, "src/Behavior.lex.tex");
    assert!(behavior.contains("isWritable := true"), "{behavior}");
    assert!(behavior.contains("isWritable := false"), "{behavior}");
}

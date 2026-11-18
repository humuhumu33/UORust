Feature: engine

  uorust reads a Cargo workspace and writes a Prism project: router proposals,
  admission that refuses ambiguity with typed refusals, and a byte-deterministic
  projection the PrismPM SDK checks.

  @UE-01 @build
  Scenario: uorust projects the fixture workspace into exactly the committed Prism project, which lexlean fmt, lexlean lock and prismpm check accept
    Given the fixture workspace in crates/uorust/tests/fixtures/thermostat
    When the suite exercises UE-01
    Then the fresh projection equals crates/uorust/tests/fixtures/thermostat-prism byte for byte

  @UE-02 @build
  Scenario: Two projections of the same workspace are byte-identical, for the fixture and for this workspace
    Given the fixture workspace and this workspace
    When the suite exercises UE-02
    Then each workspace projects to the same files with the same bytes twice

  @UE-03 @build
  Scenario: Admission refuses an ambiguous call, a feature without acceptance and a missing MSRV with typed refusals recorded as open claims, and emits no record for them
    Given a workspace whose call is ambiguous, whose enum is both an error and a state, whose functions are untested and which declares no rust-version
    When the suite exercises UE-03
    Then frontier.toml records ambiguous-call, ambiguous-fact, feature-without-acceptance and no-msrv refusals with status open
    And the Interface module declares no feature

  @UE-04 @build
  Scenario: Total functions over bool, unsigned comparisons and unit enums lift to LexLean definitions, closed assertions over them become decide theorems, and every other function is residue with a typed reason
    Given the fixture workspace
    When the suite exercises UE-04
    Then Behavior defines shouldHeat, withinBand, nextMode and halt with decide theorems from the fixture's assertions
    And frontier.toml records midpoint as arithmetic residue and parse_target as type residue

  @UE-05 @build
  Scenario: The engine's router equals model/routes.toml route for route and answers every router vector of that table exactly
    Given the route table in model/routes.toml
    When the suite exercises UE-05
    Then uorust::ROUTES lists the same routes in the same order
    And uorust::respond answers each non-workbench vector with its response

  @UE-06 @build
  Scenario: Every way a run can end without a projection is a typed UorustError variant with a negative test
    Given a missing workspace, a directory with no manifest, a malformed lockfile, a bad timestamp, a foreign output directory and a stale projection
    When the suite exercises UE-06
    Then each ends in its own UorustError variant

  @UE-07 @build
  Scenario: The gate checks the committed fixture projection and a fresh projection of this workspace with lexlean and prismpm
    Given the Justfile
    When the suite exercises UE-07
    Then vv runs the uorust recipe, which runs the three SDK checks on both projections

  @UE-08 @build
  Scenario: A public trait's methods are interface calls its module's feature names, and a method call through the trait resolves to the trait's declaration
    Given the fixture workspace, whose sensor module declares pub trait Sensor implemented by Fixed
    When the suite exercises UE-08
    Then Interface declares callThermoCoreSensorSensorRead and callThermoCoreSensorSensorCalibrated, and featureThermoCoreSensor names both and is accepted by the test that calls Fixed(20).read()

  @UE-09 @build
  Scenario: Each module with processes is its own OPM view beside its component's: the component is the object, the module's functions are its processes, and each process zooms into the processes it calls in the module
    Given the fixture workspace, whose config module's load_target calls parse_target
    When the suite exercises UE-09
    Then Design declares the view opmModuleLibThermoCoreConfig with an invocation and a zoom from load_target to parse_target, and one view per other module

  @UE-10 @build
  Scenario: The Behavior and SupplyChain modules take their titles from uorust.titles, a glossary package the projection carries and locks
    Given the fixture workspace
    When the suite exercises UE-10
    Then Behavior is titled Lifted Rust behavior and SupplyChain Locked supply chain, both use uorust.titles@1.0.0, and lexlean.toml and lexlean.lock name language/uorust.titles

  @UE-11 @build
  Scenario: Unsigned arithmetic whose overflow the code names, signed integers, strings, structs, tuples, Option, Result with ?, slices with structural recursion, integer matches and string format! lift to LexLean definitions that lexlean verify accepts, with the Lean axioms they need declared
    Given the fixture workspace, whose band module uses checked, wrapping and saturating arithmetic, i64, &str, a struct, a tuple, Option, Result, a slice and format!
    When the suite exercises UE-11
    Then every band function is a Behavior definition, plain + in midpoint stays arithmetic residue, and the uorust recipe runs lexlean verify on both projections

  @UE-12 @build
  Scenario: A RustSec advisory that affects a locked package's version is a Risk on the supply chain, answered by the advisory gate or accepted where deny.toml ignores it with its reason; one nothing answers is refused, and a run without a database records that none was consulted
    Given the advised fixture, whose Cargo.lock pins smallvec 0.6.9, with five copied RustSec advisories and a deny.toml that ignores RUSTSEC-2018-0018
    When the suite exercises UE-12
    Then Security holds a Risk for each of the four advisories that affect 0.6.9, an AcceptedRisk for the ignored one, failing gate measurements for the rest, and the SDK accepts the projection

  @UE-13 @build
  Scenario: Every feature of this workspace's own projection is admitted: each library module and binary entry point is exercised by a test, doctest or example, directly or through a helper of the test's file
    Given this workspace, whose repo-model codegen and xtask binary have their own tests
    When the suite exercises UE-13
    Then frontier.toml records no feature-without-acceptance refusal, and Interface declares featureRepoModelCodegen and featureXtaskMain

  @UE-14 @build
  Scenario: uorust run on its own source reproduces its hand model: every hand component, call, threat, control and quality measure in app/src is paired with the derived record that states the same fact or listed as outside the code with a reason, the hand route table equals uorust::ROUTES, and the drift is 0
    Given this workspace and the hand model in app/src, with crates/uorust/tests/fixpoint.toml pairing their records
    When the suite exercises UE-14
    Then every paired record on both sides states its fact, every checked hand record is paired or outside, the route tables agree, and the drift is 0

  @UE-15 @build
  Scenario: Iterator chains, inherent methods, read-only maps, early returns and for loops lift
    Given the thermostat fixture, whose roster module uses iterator chains with closures, methods taking self, BTreeMap, HashMap and BTreeSet reads, early returns, let else, tuple lets and for loops that push onto locals
    When the suite exercises UE-15
    Then every roster function and method is a LexLean definition, each chain a helper with recursive_argument, no roster function is residue, and lexlean verify passes on the committed projection

  @UE-16 @build
  Scenario: A lifted function never turns unlifted
    Given this workspace, the thermostat fixture and their committed lift registers
    When the suite exercises UE-16
    Then every function of product code counted from the source has exactly one typed row keyed by its κ, a divergence appears on lifted rows only and the register keeps it, no κ a committed register records as lifted is decided otherwise, this workspace's register is current, and a planted function whose callee stops lifting is refused by the register, by uorust write and by uorust check

  @UE-17 @build
  Scenario: The lift translates MIR mechanically
    Given the mir-path fixture, its committed verdict register and the vendored MIR definition of the pinned toolchain
    When the suite exercises UE-17
    Then every MIR kind has a coverage row, the trial projection lists a loop helper with its measure and decrease theorems and a theorem per obligation, std's own MIR instantiated at a call's types, a std model from the table with a mutable borrow returned by its backward function, and a promoted constant lowered from its own body, the mutable borrow of a parameter is a residue row naming its MIR cause, the proved translation lifts, the refused one is residue naming the verifier's diagnostic, and a verdict of another digest admits nothing

  @UE-18 @build
  Scenario: The translator certifies itself and only shrinks by hand
    Given this workspace's lift register and the certified translator's manifest, and the parent commit's
    When the suite exercises UE-18
    Then each lifted translator function is a production root, a root the compiler finds ineligible is refused with its diagnostic, every certified source matches the SHA-256 its bundle records, the share counts the translator functions reachable from the translate entry and those certified, and a commit with more uncertified translator functions than its parent is refused naming each new one and its cause

  @UE-19 @build
  Scenario: Every body of core is embedded and its evaluator checked against rustc
    Given core's MIR dumps, rust-src at the pinned toolchain and the MIR semantics module
    When the suite exercises UE-19
    Then every body is a value of the MIR datatype whose kinds are rustc's, a long list nests no deeper than its root, the constructs the evaluator does not define are read from its source, each register row records its embedding and what blocks it, and each sampled case is a theorem that the evaluator returns what rustc's build returned

  @UE-21 @build
  Scenario: A method that writes its own struct threads it
    Given a planted crate whose struct has setters taking &mut self and one taking &mut self that returns a value
    When the suite exercises UE-21
    Then each setter lifts as the function from the struct to its new value, a field write binds the record the model's threadedRecord gives with every other field projected from the value before it, the last write to a field wins, and the method that returns a value is a residue row naming its &mut self receiver

  @UE-22 @build
  Scenario: A write through a nested place threads it
    Given a planted crate whose struct holds a tuple of a fixed array and a key, with setters writing an item of the tuple and a literal index of the array, and one writing an index it computes
    When the suite exercises UE-22
    Then each setter lifts, its index write rebuilds the array with the written element and every other kept, its item write is the model's record write of the tuple, and the setter with a computed index is a residue row naming the place

  @UE-23 @build
  Scenario: Bytes, flags and unit enums convert by the prelude's definitions
    Given a planted crate whose struct holds an amount as eight little-endian bytes, a flag and a state byte, with a getter and setters converting them, and an enum whose source sets a discriminant as an expression
    When the suite exercises UE-23
    Then the getter and the setters lift to the prelude's fromLeBytes, leBytes and bitOf and to the match from each variant to its index, and the cast of the enum whose discriminant is an expression is a residue row naming them

  @UE-24 @build
  Scenario: A crate reads the types of the workspace crates it depends on
    Given a planted workspace whose second crate imports the first crate's error enum through a nested use group and names it by its full path, and whose third crate does not depend on the first
    When the suite exercises UE-24
    Then the second crate's functions that return or build the enum lift, and the third crate's function that names it is a residue row naming the type

  @UE-25 @build
  Scenario: An error converts by its From implementation
    Given a planted crate whose function returning a program error applies `?` to a result of a token error, whose other function returns a token error `into` a program error, an implementation of From of the token error for the program error, and a function applying `?` where no conversion exists
    When the suite exercises UE-25
    Then the first two functions lift, each error path applying the lifted from, and the third is a residue row naming the `?` of another type

  @UE-26 @build
  Scenario: Self and its associated types name the impl
    Given a planted crate whose enum implements TryFrom of a byte returning Result of Self and Self::Error, a struct method calling that try_from on its byte field, and another calling it on a flag
    When the suite exercises UE-26
    Then the try_from and the method calling it on the byte lift, and the method calling it on a flag is a residue row naming try_from

  @UE-27 @build
  Scenario: An integer read as a fieldless enum is its variant
    Given a planted crate whose enum sets discriminants 5 and 9 and leaves the next unset, a function transmuting a byte matched by 5 or 9..=10 into it, another transmuting a byte matched by 5..=6, and a cast of the enum to a byte
    When the suite exercises UE-27
    Then the first transmuting function and the cast lift, the cast giving each variant its discriminant, and the second transmuting function is a residue row naming the unsafe block

  @UE-28 @build
  Scenario: Account metas and branch hints lift by their models
    Given a planted crate depending on solana-instruction, solana-pubkey and pinocchio, with a function pushing AccountMeta::new and AccountMeta::new_readonly onto a vector and another testing likely and unlikely
    When the suite exercises UE-28
    Then both functions lift, the metas writable and read-only as their constructors set them

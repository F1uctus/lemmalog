//! The annotation parameter, exercised at carriers that are not `Ann`.
//!
//! Upstream's own suite runs the default instantiation, so it proves the
//! parameterisation changed nothing - but it never builds an `Engine<A>` for
//! any other `A`, and so it cannot see whether the trait hooks are actually
//! wired into the fixpoint, what the evaluator hands them, or what the trait's
//! own defaults do when a carrier leans on them.

use std::collections::BTreeSet;

use lemmalog::{eval::Key, Annotation, Engine};

/// A carrier that counts the body atoms multiplied into an annotation and
/// records every derivation stamped onto it, and that **deliberately
/// implements no [`lemmalog::Interpret`]**.
///
/// That absence is an assertion, not an omission. The evaluator is bounded by
/// `A: Annotation`, and `Interpret<V>: Annotation` is a subtrait of it, so a
/// bound on `Annotation` grants nothing about `Interpret`. If any evaluation
/// path could read an annotation through `interpret`, `Engine<A>` would need
/// an `Interpret` bound and this file would stop compiling. It compiling is
/// the proof that a read-time interpretation - an arithmetic mean, a median,
/// anything the fixpoint could not lawfully fold - cannot enter the fixpoint.
#[derive(Debug, Clone, PartialEq)]
struct Tally {
    atoms: u32,
    derivations: BTreeSet<String>,
}

impl Annotation for Tally {
    fn one() -> Self {
        Tally {
            atoms: 0,
            derivations: BTreeSet::new(),
        }
    }

    fn zero() -> Self {
        Tally {
            atoms: 0,
            derivations: BTreeSet::from(["<zero>".to_owned()]),
        }
    }

    fn times(&self, other: &Self) -> Self {
        // law 4: zero annihilates. Cheap here, and this carrier is the worked
        // example a future carrier author will copy.
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }

        Tally {
            atoms: self.atoms + other.atoms,
            derivations: self.derivations.union(&other.derivations).cloned().collect(),
        }
    }

    fn plus(&self, other: &Self) -> Self {
        Tally {
            atoms: self.atoms.max(other.atoms),
            derivations: self.derivations.union(&other.derivations).cloned().collect(),
        }
    }

    fn derive(mut self, rule: &str, body: &[Key]) -> Self {
        let preds: Vec<&str> = body.iter().map(|(pred, _)| pred.as_str()).collect();
        self.derivations.insert(format!("{rule}[{}]", preds.join(",")));

        self
    }
}

/// A carrier that implements only the four methods the trait requires, so that
/// what the trait's *defaults* do on their own is visible rather than masked by
/// a carrier that happens to override them.
#[derive(Debug, Clone, PartialEq)]
struct Minimal(u32);

impl Annotation for Minimal {
    fn one() -> Self {
        Minimal(1)
    }

    fn zero() -> Self {
        Minimal(0)
    }

    fn times(&self, other: &Self) -> Self {
        Minimal(self.0 * other.0)
    }

    fn plus(&self, other: &Self) -> Self {
        Minimal(self.0.max(other.0))
    }
}

#[test]
fn a_carrier_the_evaluator_cannot_interpret_still_drives_the_whole_fixpoint() {
    let mut engine: Engine<Tally> = Engine::default();
    engine
        .install_program("place: place_of(P, H) :- runs_in(P, C), hosted_by(C, H).")
        .expect("the program parses");

    let scribe = engine.sym("scribe");
    let atlas = engine.sym("atlas");
    let borealis = engine.sym("borealis");
    engine.declare("runs_in", &[scribe, atlas], Tally::base());
    engine.declare("hosted_by", &[atlas, borealis], Tally::base());
    engine.run();

    let fact = engine
        .fact("place_of", &[scribe, borealis])
        .expect("the rule fires");

    // `atoms: 2` is one() (0) times each body fact (1 each), so a `times` the
    // evaluator failed to call reads as a different number rather than as
    // silence.
    //
    // The two `derivations` entries are the load-bearing half. This is ONE
    // logical derivation, and `derive` is handed it TWICE, once per positive
    // body atom position, with the body keys in FIRING ORDER - the delta atom
    // first, the rest in body order. The evaluator does not sort them and
    // never has.
    //
    // Any carrier whose reading counts distinct derivations - which is every
    // reason `derive` exists - must sort the keys itself before fingerprinting
    // them, or one logical derivation is counted once per permutation of its
    // body and a mean over derivations becomes permutation-weighted. If a
    // future change sorts at the call site instead, this set collapses to one
    // entry and this assertion is what says so.
    assert_eq!(
        fact.ann,
        Tally {
            atoms: 2,
            derivations: BTreeSet::from([
                "place[hosted_by,runs_in]".to_owned(),
                "place[runs_in,hosted_by]".to_owned(),
            ]),
        }
    );
}

#[test]
fn the_trait_defaults_alone_reproduce_negation_as_absence() {
    let mut engine: Engine<Minimal> = Engine::default();
    engine
        .install_program("ok: safe(P) :- pkg(P), !banned(P).")
        .expect("the program parses");

    let alpha = engine.sym("alpha");
    let beta = engine.sym("beta");
    engine.declare("pkg", &[alpha], Minimal::one());
    engine.declare("pkg", &[beta], Minimal::one());
    engine.declare("banned", &[alpha], Minimal::one());
    engine.run();

    // Pruning is decided by `is_zero`, not by what `negate` returns, so a
    // carrier that takes every default it is offered still has to prune on a
    // blocked body. `Engine<Ann>` derives no `safe(alpha)` and neither may this.
    assert!(
        engine.fact("safe", &[alpha]).is_none(),
        "a banned package must not be safe"
    );
    assert_eq!(
        engine
            .fact("safe", &[beta])
            .expect("an unbanned package is safe")
            .ann,
        Minimal(1),
        "the rule fires at all, so the absence above is pruning and not a dead program"
    );
}

impl Tally {
    /// One base fact, as the caller asserts it.
    fn base() -> Self {
        Tally {
            atoms: 1,
            derivations: BTreeSet::new(),
        }
    }
}

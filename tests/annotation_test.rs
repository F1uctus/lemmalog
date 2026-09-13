//! The annotation parameter, exercised at a carrier that is not `Ann`.
//!
//! Upstream's own suite runs the default instantiation, so it proves the
//! parameterisation changed nothing - but it never builds an `Engine<A>` for
//! any other `A`, and so it cannot see whether the trait hooks are actually
//! wired into the fixpoint or merely declared next to it.

use lemmalog::{eval::Key, Annotation, Engine};

/// A carrier that counts the body atoms multiplied into an annotation and
/// records the rules stamped onto it, and that **deliberately implements no
/// [`lemmalog::Interpret`]**.
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
    rules: Vec<String>,
}

impl Annotation for Tally {
    fn one() -> Self {
        Tally {
            atoms: 0,
            rules: Vec::new(),
        }
    }

    fn zero() -> Self {
        Tally {
            atoms: 0,
            rules: vec!["<zero>".to_owned()],
        }
    }

    fn times(&self, other: &Self) -> Self {
        let mut rules = self.rules.clone();
        rules.extend(other.rules.iter().cloned());

        Tally {
            atoms: self.atoms + other.atoms,
            rules,
        }
    }

    fn plus(&self, other: &Self) -> Self {
        if other.atoms > self.atoms {
            other.clone()
        } else {
            self.clone()
        }
    }

    fn is_zero(&self) -> bool {
        *self == Self::zero()
    }

    fn derive(mut self, rule: &str, _body: &[Key]) -> Self {
        self.rules.push(rule.to_owned());

        self
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

    // one() contributes 0 atoms, each body fact contributes its own 1, and
    // emit_head stamps the clause name through `derive` before the head is
    // summed in. Every one of those numbers comes from a different hook, so a
    // hook the evaluator failed to call shows up as a different value here.
    assert_eq!(
        fact.ann,
        Tally {
            atoms: 2,
            rules: vec!["place".to_owned()]
        }
    );
}

impl Tally {
    /// One base fact, as the caller asserts it.
    fn base() -> Self {
        Tally {
            atoms: 1,
            rules: Vec::new(),
        }
    }
}

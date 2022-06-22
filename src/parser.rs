//! Turn a string of markdown into events.

// To do: this should start with `containers`, when they’re done.
use crate::content::flow::flow;
use crate::tokenizer::{as_codes, Code, Event, Point};

/// Turn a string of markdown into events.
///
/// Passes the codes back so the compiler can access the source.
pub fn parse(value: &str) -> (Vec<Event>, Vec<Code>) {
    let codes = as_codes(value);
    // To do: pass a reference to this around, and slices in the (back)feeding. Might be tough.
    let events = flow(
        &codes,
        Point {
            line: 1,
            column: 1,
            offset: 0,
        },
        0,
    );
    (events, codes)
}

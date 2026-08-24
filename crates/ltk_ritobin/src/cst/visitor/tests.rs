use super::*;
use crate::cst::Kind;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Event {
    Enter(Kind),
    Token,
    Exit(Kind),
}

/// Records every callback; optionally returns Abort/Stop/Skip when a node of
/// the configured kind is hit.
#[derive(Default)]
struct Recorder {
    events: Vec<Event>,
    abort_on_enter: Option<Kind>,
    stop_on_enter: Option<Kind>,
    skip_on_enter: Option<Kind>,
    abort_on_exit: Option<Kind>,
    stop_on_exit: Option<Kind>,
    skip_on_exit: Option<Kind>,
    abort_on_token: bool,
    stop_on_token: bool,
    skip_on_token_in: Option<Kind>,
}

impl Recorder {
    fn walk(mut self, text: &str) -> Vec<Event> {
        let cst = Cst::parse(text);
        assert!(cst.errors.is_empty(), "parse errors: {:#?}", cst.errors);
        cst.walk(&mut self);
        self.events
    }
}

impl Visitor for Recorder {
    fn enter_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
        let kind = ctx.node(tree).unwrap().kind;
        self.events.push(Event::Enter(kind));
        if self.abort_on_enter == Some(kind) {
            return Visit::Abort;
        }
        if self.stop_on_enter == Some(kind) {
            return Visit::Stop;
        }
        if self.skip_on_enter == Some(kind) {
            return Visit::Skip;
        }
        Visit::Continue
    }
    fn exit_tree(&mut self, ctx: &VisitCtx<'_>, tree: NodeId) -> Visit {
        let kind = ctx.node(tree).unwrap().kind;
        self.events.push(Event::Exit(kind));
        if self.abort_on_exit == Some(kind) {
            return Visit::Abort;
        }
        if self.stop_on_exit == Some(kind) {
            return Visit::Stop;
        }
        if self.skip_on_exit == Some(kind) {
            return Visit::Skip;
        }
        Visit::Continue
    }
    fn visit_token(&mut self, ctx: &VisitCtx<'_>, _token: TokenId, parent: NodeId) -> Visit {
        self.events.push(Event::Token);
        if self.abort_on_token {
            return Visit::Abort;
        }
        if self.stop_on_token {
            return Visit::Stop;
        }
        if self.skip_on_token_in == Some(ctx.node(parent).unwrap().kind) {
            return Visit::Skip;
        }
        Visit::Continue
    }
}

const TEXT: &str = "a: list[u32] = { 1 2 }\nb: u32 = 3";

/// Every `Enter` has a matching, properly nested `Exit`, and nothing is
/// left open at the end.
fn assert_balanced(events: &[Event]) {
    let mut stack = Vec::new();
    for event in events {
        match event {
            Event::Enter(kind) => stack.push(*kind),
            Event::Exit(kind) => {
                assert_eq!(stack.pop(), Some(*kind), "mismatched exit in {events:#?}")
            }
            Event::Token => {}
        }
    }
    assert!(stack.is_empty(), "nodes never exited: {stack:?}");
}

fn count(events: &[Event], event: Event) -> usize {
    events.iter().filter(|e| **e == event).count()
}

#[test]
fn full_walk_is_balanced() {
    let events = Recorder::default().walk(TEXT);
    assert_balanced(&events);
    assert_eq!(events.first(), Some(&Event::Enter(Kind::File)));
    assert_eq!(events.last(), Some(&Event::Exit(Kind::File)));
    assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
}

#[test]
fn stop_from_enter_exits_open_ancestors() {
    let events = Recorder {
        stop_on_enter: Some(Kind::EntryValue),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    // nothing new is entered after the stop fired
    let stop_at = events
        .iter()
        .position(|e| *e == Event::Enter(Kind::EntryValue))
        .unwrap();
    assert!(
        events[stop_at + 1..]
            .iter()
            .all(|e| matches!(e, Event::Exit(_))),
        "walk continued after Stop: {events:#?}"
    );
}

#[test]
fn stop_from_token_exits_open_ancestors() {
    let events = Recorder {
        stop_on_token: true,
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    assert_eq!(count(&events, Event::Token), 1);
}

#[test]
fn stop_from_exit_exits_open_ancestors() {
    let events = Recorder {
        stop_on_exit: Some(Kind::EntryKey),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    let stop_at = events
        .iter()
        .position(|e| *e == Event::Exit(Kind::EntryKey))
        .unwrap();
    assert!(
        events[stop_at + 1..]
            .iter()
            .all(|e| matches!(e, Event::Exit(_))),
        "walk continued after Stop: {events:#?}"
    );
}

#[test]
fn skip_from_enter_skips_children_but_still_exits() {
    let events = Recorder {
        skip_on_enter: Some(Kind::Entry),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    // children of both entries were skipped, the entries still exited,
    // and the walk went on to the sibling entry
    assert_eq!(count(&events, Event::Enter(Kind::EntryKey)), 0);
    assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
    assert_eq!(count(&events, Event::Exit(Kind::Entry)), 2);
}

#[test]
fn skip_from_token_skips_the_nodes_remaining_children() {
    // skip fires on the block's `{`, so its list items are never entered
    let events = Recorder {
        skip_on_token_in: Some(Kind::Block),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    assert_eq!(count(&events, Event::Enter(Kind::ListItem)), 0);
    assert_eq!(count(&events, Event::Exit(Kind::Block)), 1);
}

#[test]
fn skip_from_exit_prunes_later_siblings() {
    let events = Recorder {
        skip_on_exit: Some(Kind::Entry),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    // the first entry's exit returned Skip, so the second entry is pruned,
    // but the parent still exits normally
    assert_eq!(count(&events, Event::Enter(Kind::Entry)), 1);
    assert_eq!(events.last(), Some(&Event::Exit(Kind::File)));
}

#[test]
fn pruning_is_local_and_the_walk_continues_elsewhere() {
    // the first list item's exit prunes the block's remaining children,
    // but everything outside the block is still walked
    let events = Recorder {
        skip_on_exit: Some(Kind::ListItem),
        ..Default::default()
    }
    .walk(TEXT);
    assert_balanced(&events);
    assert_eq!(count(&events, Event::Enter(Kind::ListItem)), 1);
    assert_eq!(count(&events, Event::Enter(Kind::Entry)), 2);
}

#[test]
fn abort_from_enter_runs_no_more_callbacks() {
    let events = Recorder {
        abort_on_enter: Some(Kind::EntryValue),
        ..Default::default()
    }
    .walk(TEXT);
    // the aborting node and its open ancestors never exit
    assert_eq!(events.last(), Some(&Event::Enter(Kind::EntryValue)));
    assert_eq!(count(&events, Event::Exit(Kind::Entry)), 0);
    assert_eq!(count(&events, Event::Exit(Kind::File)), 0);
}

#[test]
fn abort_from_token_runs_no_more_callbacks() {
    let events = Recorder {
        abort_on_token: true,
        ..Default::default()
    }
    .walk(TEXT);
    assert_eq!(count(&events, Event::Token), 1);
    assert_eq!(events.last(), Some(&Event::Token));
}

#[test]
fn abort_from_exit_skips_the_remaining_exits() {
    let events = Recorder {
        abort_on_exit: Some(Kind::EntryKey),
        ..Default::default()
    }
    .walk(TEXT);
    assert_eq!(events.last(), Some(&Event::Exit(Kind::EntryKey)));
    assert_eq!(count(&events, Event::Exit(Kind::Entry)), 0);
    assert_eq!(count(&events, Event::Exit(Kind::File)), 0);
}

#[test]
fn abort_from_exit_while_unwinding_skips_the_remaining_exits() {
    // a Stop deep in the tree starts unwinding; an ancestor's exit aborts,
    // so the exits above it never run
    let events = Recorder {
        stop_on_enter: Some(Kind::EntryValue),
        abort_on_exit: Some(Kind::Entry),
        ..Default::default()
    }
    .walk(TEXT);
    assert_eq!(events.last(), Some(&Event::Exit(Kind::Entry)));
    assert_eq!(count(&events, Event::Exit(Kind::File)), 0);
}

#[test]
fn walk_reports_how_it_ended() {
    let cst = Cst::parse(TEXT);
    assert_eq!(cst.walk(&mut Recorder::default()), WalkOutcome::Completed);
    assert_eq!(
        cst.walk(&mut Recorder {
            stop_on_token: true,
            ..Default::default()
        }),
        WalkOutcome::Stopped
    );
    assert_eq!(
        cst.walk(&mut Recorder {
            abort_on_token: true,
            ..Default::default()
        }),
        WalkOutcome::Aborted
    );
    // pruning is not a stop
    assert_eq!(
        cst.walk(&mut Recorder {
            skip_on_exit: Some(Kind::Entry),
            ..Default::default()
        }),
        WalkOutcome::Completed
    );
}

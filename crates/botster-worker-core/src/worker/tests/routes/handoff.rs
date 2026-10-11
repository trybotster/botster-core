//! DP-2, plan revision 23ae: the descriptors of a link read belong to the one `AttachRoute` frame whose first byte is in
//! that read. Each proof runs on both read forms that the kernels give (`Form`), and checks that every descriptor is
//! closed exactly once or taken by a route.

use super::*;

/// Where a read with a descriptor begins and ends (measured, plan 23ae). In both forms the descriptor's read holds the first
/// byte of the `AttachRoute` frame, and the frame goes on in later reads.
#[derive(Debug, Clone, Copy)]
enum Form {
    /// macOS stops a read before a descriptor segment: the descriptor's read begins at the frame's first byte.
    StartsAtTheFrame,
    /// Linux reads on into a descriptor segment: the descriptor's read begins with the tail of an earlier frame.
    StartsWithAnEarlierTail,
}

const FORMS: [Form; 2] = [Form::StartsAtTheFrame, Form::StartsWithAnEarlierTail];

fn attach_frame(route: u64) -> Vec<u8> {
    World::msg(&HostMsg::AttachRoute {
        route: RouteId(route),
        options: options(),
        limits: limits(),
    })
}

/// A frame that is not an `AttachRoute`: a cancel of a request that the worker does not hold, which changes nothing.
fn other_frame() -> Vec<u8> {
    World::msg(&HostMsg::Cancel { req: 999 })
}

/// The link reads of `earlier`, then `frame` with `descriptors` on the read that holds its first byte, then `later`, in
/// `form`. The descriptor's read holds `frame`'s first `prefix` bytes; the rest of `frame` and `later` come in reads of
/// `chunk` bytes.
fn reads(
    form: Form,
    earlier: &[u8],
    frame: &[u8],
    descriptors: &[DescriptorId],
    later: &[u8],
    prefix: usize,
    chunk: usize,
) -> Vec<Input> {
    let mut out = Vec::new();
    let mut with_descriptor = Vec::new();
    match form {
        Form::StartsAtTheFrame => out.push(World::read_plain(earlier.to_vec())),
        Form::StartsWithAnEarlierTail => {
            let split = earlier.len() / 2;
            out.push(World::read_plain(earlier[..split].to_vec()));
            with_descriptor.extend_from_slice(&earlier[split..]);
        }
    }
    with_descriptor.extend_from_slice(&frame[..prefix]);
    out.push(World::read(with_descriptor, descriptors.to_vec()));
    let rest = [&frame[prefix..], later].concat();
    out.extend(rest.chunks(chunk).map(|c| World::read_plain(c.to_vec())));
    out
}

fn feed_all(w: &mut World, inputs: Vec<Input>) -> Vec<Action> {
    let mut actions = Vec::new();
    for input in inputs {
        actions.extend(w.feed(input));
    }
    actions
}

/// How many times each descriptor is closed in `actions`: by `CloseDescriptor`, or by the `RouteClose` of the route that
/// took it.
fn descriptor_closes(actions: &[Action]) -> BTreeMap<DescriptorId, usize> {
    let mut bound = BTreeMap::new();
    let mut closes = BTreeMap::new();
    for action in actions {
        match action {
            Action::BindRoute { descriptor, route } => {
                bound.insert(*route, *descriptor);
            }
            Action::CloseDescriptor(id) => *closes.entry(*id).or_insert(0) += 1,
            Action::RouteClose { route } => {
                if let Some(id) = bound.get(route) {
                    *closes.entry(*id).or_insert(0) += 1;
                }
            }
            _ => {}
        }
    }
    closes
}

fn binds(actions: &[Action]) -> Vec<(DescriptorId, RouteId)> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::BindRoute { descriptor, route } => Some((*descriptor, *route)),
            _ => None,
        })
        .collect()
}

/// The actions of a fault, with the link's close: the close waits until the bytes already sent are written (LC-7).
fn with_link_written(w: &mut World, mut actions: Vec<Action>) -> Vec<Action> {
    actions.extend(w.feed(Input::LinkWritten { total: u64::MAX }));
    actions
}

/// An `AttachRoute` frame that ends in later reads than its descriptor's read takes that descriptor, on both forms, with
/// a frame before it and after it.
#[test]
fn a_split_attach_route_takes_the_descriptor_of_the_read_with_its_first_byte() {
    for form in FORMS {
        for (prefix, chunk) in [(1, 3), (3, 3), (10, 1000)] {
            let mut w = World::running();
            let inputs = reads(
                form,
                &other_frame(),
                &attach_frame(1),
                &[DescriptorId(1)],
                &other_frame(),
                prefix,
                chunk,
            );
            let actions = feed_all(&mut w, inputs);
            assert_eq!(
                binds(&actions),
                vec![(DescriptorId(1), RouteId(1))],
                "{form:?} {prefix} {chunk}"
            );
            assert!(
                !actions.contains(&Action::LinkClose),
                "{form:?}: {actions:?}"
            );
            assert!(descriptor_closes(&actions).is_empty(), "{form:?}");
        }
    }
}

/// Plan 23ae (lead precision 1): the descriptor's read begins with the tail of an earlier frame and then holds a whole other
/// frame before the `AttachRoute` frame's first byte; the descriptor is still the `AttachRoute` frame's.
#[test]
fn a_descriptor_read_that_begins_with_earlier_frames_gives_its_descriptor_to_the_attach_route() {
    let mut w = World::running();
    let earlier = [other_frame(), other_frame()].concat();
    let inputs = reads(
        Form::StartsWithAnEarlierTail,
        &earlier,
        &attach_frame(1),
        &[DescriptorId(1)],
        &[],
        4,
        3,
    );
    let actions = feed_all(&mut w, inputs);
    assert_eq!(binds(&actions), vec![(DescriptorId(1), RouteId(1))]);
    assert!(!actions.contains(&Action::LinkClose), "{actions:?}");
}

/// Two `AttachRoute` frames, each sent with its own descriptor, take their own descriptors in order, on both forms.
#[test]
fn two_attach_routes_take_their_own_descriptors() {
    for form in FORMS {
        let mut w = World::running();
        let mut inputs = reads(
            form,
            &other_frame(),
            &attach_frame(1),
            &[DescriptorId(1)],
            &[],
            2,
            3,
        );
        inputs.extend(reads(
            form,
            &other_frame(),
            &attach_frame(2),
            &[DescriptorId(2)],
            &other_frame(),
            2,
            3,
        ));
        let actions = feed_all(&mut w, inputs);
        assert_eq!(
            binds(&actions),
            vec![(DescriptorId(1), RouteId(1)), (DescriptorId(2), RouteId(2))],
            "{form:?}"
        );
        assert!(!actions.contains(&Action::LinkClose), "{form:?}");
    }
}

/// A fault: a read with a descriptor holds the first byte of no `AttachRoute` frame. The link closes, and the descriptor
/// is closed once.
#[test]
fn a_descriptor_with_no_attach_route_start_in_its_read_breaks_the_link() {
    for form in FORMS {
        let mut w = World::running();
        let inputs = reads(
            form,
            &other_frame(),
            &other_frame(),
            &[DescriptorId(1)],
            &[],
            2,
            3,
        );
        let fed = feed_all(&mut w, inputs);
        let actions = with_link_written(&mut w, fed);
        assert!(
            actions.contains(&Action::LinkClose),
            "{form:?}: {actions:?}"
        );
        assert!(binds(&actions).is_empty(), "{form:?}");
        assert_eq!(
            descriptor_closes(&actions),
            BTreeMap::from([(DescriptorId(1), 1)]),
            "{form:?}"
        );
    }
}

/// A fault: the `AttachRoute` frame starts in a read with no descriptor, and the descriptor comes in a later read of the
/// frame. That descriptor is not the frame's: the link closes, and the descriptor is closed once.
#[test]
fn an_attach_route_whose_first_byte_came_with_no_descriptor_breaks_the_link() {
    let mut w = World::running();
    let frame = attach_frame(1);
    let mut actions = w.feed(World::read_plain(frame[..3].to_vec()));
    actions.extend(w.feed(World::read(frame[3..].to_vec(), vec![DescriptorId(1)])));
    let actions = with_link_written(&mut w, actions);
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert!(binds(&actions).is_empty());
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
}

/// A fault: the read of an `AttachRoute` frame carries two descriptors, not the frame's one stream. The link closes, and
/// each descriptor is closed once.
#[test]
fn an_attach_route_read_with_two_descriptors_breaks_the_link() {
    for form in FORMS {
        let mut w = World::running();
        let inputs = reads(
            form,
            &other_frame(),
            &attach_frame(1),
            &[DescriptorId(1), DescriptorId(2)],
            &[],
            2,
            3,
        );
        let fed = feed_all(&mut w, inputs);
        let actions = with_link_written(&mut w, fed);
        assert!(
            actions.contains(&Action::LinkClose),
            "{form:?}: {actions:?}"
        );
        assert!(binds(&actions).is_empty(), "{form:?}");
        assert_eq!(
            descriptor_closes(&actions),
            BTreeMap::from([(DescriptorId(1), 1), (DescriptorId(2), 1)]),
            "{form:?}"
        );
    }
}

/// A fault found after a bind (lead ruling Q1): a read with one descriptor holds the first bytes of two `AttachRoute`
/// frames. The first takes the descriptor in frame order; the second breaks the link, and the first route ends
/// `HandoffFailed`: its transport closes once, and the host is told once.
#[test]
fn a_second_attach_route_start_in_a_descriptor_read_ends_the_route_that_took_it() {
    for form in FORMS {
        let mut w = World::running();
        let both = [attach_frame(1), attach_frame(2)].concat();
        let inputs = reads(
            form,
            &other_frame(),
            &both,
            &[DescriptorId(1)],
            &[],
            both.len(),
            3,
        );
        let fed = feed_all(&mut w, inputs);
        let actions = with_link_written(&mut w, fed);
        assert_eq!(
            binds(&actions),
            vec![(DescriptorId(1), RouteId(1))],
            "{form:?}"
        );
        assert!(
            actions.contains(&Action::LinkClose),
            "{form:?}: {actions:?}"
        );
        assert_eq!(
            descriptor_closes(&actions),
            BTreeMap::from([(DescriptorId(1), 1)]),
            "{form:?}"
        );
        let closed: Vec<WorkerMsg> = w
            .reports(&actions)
            .into_iter()
            .filter(|m| matches!(m, WorkerMsg::RouteClosed { .. }))
            .collect();
        assert_eq!(
            closed,
            vec![WorkerMsg::RouteClosed {
                route: RouteId(1),
                reason: RouteCloseReason::HandoffFailed,
                route_tag: None,
            }],
            "{form:?}"
        );
    }
}

/// A fault: a descriptor before the hello passes. The link closes, and the descriptor is closed once.
#[test]
fn a_descriptor_before_the_hello_breaks_the_link() {
    let mut w = World::new();
    w.initial();
    let hello = World::host_hello(instance(), EPOCH, TOKEN);
    let actions = w.feed(World::read(hello, vec![DescriptorId(1)]));
    let actions = with_link_written(&mut w, actions);
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
}

/// DP-8: a descriptor that comes on a link that already ended is closed once, and its bytes are not read.
#[test]
fn a_descriptor_on_an_ended_link_is_closed() {
    let mut w = World::running();
    w.feed(Input::LinkClosed);
    let actions = w.feed(World::read(attach_frame(1), vec![DescriptorId(1)]));
    assert!(binds(&actions).is_empty());
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
}

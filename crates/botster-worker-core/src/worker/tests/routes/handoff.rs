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

        client: ClientId("c".into()),
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

fn binds(actions: &[Action]) -> Vec<(DescriptorId, RouteKey)> {
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
                vec![(DescriptorId(1), RouteKey(1))],
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
    assert_eq!(binds(&actions), vec![(DescriptorId(1), RouteKey(1))]);
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
            vec![
                (DescriptorId(1), RouteKey(1)),
                (DescriptorId(2), RouteKey(2))
            ],
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
            vec![(DescriptorId(1), RouteKey(1))],
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

/// F117: a read with a descriptor whose frame is above the link's bound closes the link (plan section 3), and the
/// descriptor is closed once, on both forms.
#[test]
fn a_descriptor_read_with_a_frame_above_the_bound_closes_the_descriptor() {
    let mut header = Vec::new();
    header.extend_from_slice(&(spec().link_frame_bound + 1).to_le_bytes());
    header.push(FrameType::HOST_MSG.0);
    for form in FORMS {
        let mut w = World::running();
        let inputs = reads(
            form,
            &other_frame(),
            &header,
            &[DescriptorId(1)],
            &[],
            header.len(),
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
            "{form:?}: {actions:?}"
        );
    }
}

/// F119: a frame that is not an `AttachRoute` is split over two reads, and each read carries a descriptor. Neither read
/// holds an `AttachRoute` start, so the link closes, and each descriptor is closed once.
#[test]
fn two_descriptor_reads_of_one_other_frame_close_each_descriptor_once() {
    let mut w = World::running();
    let frame = other_frame();
    let mut actions = w.feed(World::read(frame[..3].to_vec(), vec![DescriptorId(1)]));
    actions.extend(w.feed(World::read(frame[3..].to_vec(), vec![DescriptorId(2)])));
    let actions = with_link_written(&mut w, actions);
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert!(binds(&actions).is_empty());
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1), (DescriptorId(2), 1)]),
        "{actions:?}"
    );
}

/// The route closes of `actions`, as the host is told them.
fn route_closes(w: &mut World, actions: &[Action]) -> Vec<WorkerMsg> {
    w.reports(actions)
        .into_iter()
        .filter(|m| matches!(m, WorkerMsg::RouteClosed { .. }))
        .collect()
}

fn handoff_failed(route: u64) -> WorkerMsg {
    WorkerMsg::RouteClosed {
        route: RouteId(route),
        reason: RouteCloseReason::HandoffFailed,
        route_tag: None,
    }
}

/// F117, the bound case: a descriptor read holds a complete `AttachRoute` frame, then a frame above the link's bound. The
/// route takes the descriptor, and the frame above the bound closes the link before the read is checked: the route ends
/// `HandoffFailed` once (lead ruling Q1), on both forms, with no `LinkClosed` input.
#[test]
fn a_frame_above_the_bound_after_a_bound_attach_route_ends_the_route() {
    let mut header = Vec::new();
    header.extend_from_slice(&(spec().link_frame_bound + 1).to_le_bytes());
    header.push(FrameType::HOST_MSG.0);
    let both = [attach_frame(1), header].concat();
    for form in FORMS {
        let mut w = World::running();
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
            vec![(DescriptorId(1), RouteKey(1))],
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
        assert_eq!(
            route_closes(&mut w, &actions),
            vec![handoff_failed(1)],
            "{form:?}"
        );
    }
}

/// F117, a frame that closes the link: a descriptor read holds a complete `AttachRoute` frame, then a `Remove` of a
/// worker whose payload ended, which closes the link before the read is checked. The route ends `HandoffFailed` once.
#[test]
fn a_frame_that_closes_the_link_after_a_bound_attach_route_ends_the_route() {
    let mut w = World::running();
    w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    w.feed(Input::PtyDrained);
    let bytes = [attach_frame(1), World::msg(&HostMsg::Remove)].concat();
    let fed = w.feed(World::read(bytes, vec![DescriptorId(1)]));
    let actions = with_link_written(&mut w, fed);
    assert_eq!(binds(&actions), vec![(DescriptorId(1), RouteKey(1))]);
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
    assert_eq!(route_closes(&mut w, &actions), vec![handoff_failed(1)]);
}

/// A descriptor read that holds a complete `AttachRoute` frame and the first bytes of another frame is not checked until
/// that frame is complete. The link of each world ends before it is.
fn bound_with_an_open_frame(w: &mut World) -> Vec<Action> {
    let bytes = [attach_frame(1), other_frame()[..3].to_vec()].concat();
    w.feed(World::read(bytes, vec![DescriptorId(1)]))
}

/// F117: a terminate that ends the link at once, before the read of a bound route is checked, ends the route
/// `HandoffFailed` once.
#[test]
fn a_terminate_before_the_read_is_checked_ends_its_route() {
    let mut w = World::running();
    w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    w.feed(Input::PtyDrained);
    let mut actions = bound_with_an_open_frame(&mut w);
    actions.extend(w.feed(Input::Terminate));
    assert_eq!(binds(&actions), vec![(DescriptorId(1), RouteKey(1))]);
    assert!(actions.contains(&Action::LinkClose), "{actions:?}");
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
    assert_eq!(route_closes(&mut w, &actions), vec![handoff_failed(1)]);
}

/// DP-8: a link that ends before the read of a bound route is checked ends that route once; the route of a checked read
/// stays.
#[test]
fn a_link_end_before_the_read_is_checked_ends_only_its_route() {
    let mut w = World::running();
    let mut actions = w.feed(World::read(attach_frame(2), vec![DescriptorId(2)]));
    actions.extend(bound_with_an_open_frame(&mut w));
    actions.extend(w.feed(Input::LinkClosed));
    assert_eq!(
        binds(&actions),
        // Route 2 attaches first, so it has the first key.
        vec![
            (DescriptorId(2), RouteKey(1)),
            (DescriptorId(1), RouteKey(2))
        ]
    );
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
    assert!(!actions.contains(&Action::RouteClose { route: RouteKey(1) }));
}

/// F120, DP-8: a host adopts the worker while a read of the old link with a bound route is not checked. The new host gets
/// the worker's hello first, then `Adopted`, then the route's one `HandoffFailed`; the new link obeys the new host.
#[test]
fn an_adoption_before_the_read_is_checked_ends_its_route_after_the_hello() {
    let mut w = World::running();
    let mut actions = bound_with_an_open_frame(&mut w);
    actions.extend(adopt(&mut w, EPOCH + 3));
    assert_eq!(binds(&actions), vec![(DescriptorId(1), RouteKey(1))]);
    assert_eq!(
        descriptor_closes(&actions),
        BTreeMap::from([(DescriptorId(1), 1)])
    );
    let (_, frames) = after_fence(&mut w, &actions);
    assert_eq!(frames.len(), 3, "{frames:?}");
    assert_eq!(frames[0].kind, FrameType::HELLO);
    let hello = Hello::decode(&frames[0].payload).unwrap();
    assert_eq!(hello.proof, token_proof(&TOKEN, &instance(), EPOCH + 3));
    assert!(matches!(
        WorkerMsg::decode(&frames[1].payload).unwrap(),
        WorkerMsg::Adopted { .. }
    ));
    assert_eq!(
        WorkerMsg::decode(&frames[2].payload).unwrap(),
        handoff_failed(1)
    );
    let stop = w.send(&HostMsg::Stop);
    assert_eq!(signals(&stop), [SIGTERM]);
}

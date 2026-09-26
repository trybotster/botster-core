//! Lane isolation: a full lane never takes another lane's room.

use super::{Lane, LaneBounds, Outbound, Refused};

#[test]
fn a_full_invoke_lane_still_admits_cancel_and_shutdown() {
    let outbound = Outbound::new(LaneBounds::derived(2));
    outbound
        .push(Lane::Invoke, vec![1; 10])
        .expect("first invoke");
    outbound
        .push(Lane::Invoke, vec![2; 10])
        .expect("second invoke");

    assert_eq!(
        outbound.push(Lane::Invoke, vec![3; 10]),
        Err(Refused::Full),
        "the invoke lane is bounded by the in-flight invocations"
    );
    outbound.push(Lane::Cancel, vec![4]).expect("cancel room");
    outbound.push(Lane::Cancel, vec![5]).expect("cancel room");
    outbound
        .push(Lane::Shutdown, vec![6])
        .expect("shutdown room");
    assert_eq!(outbound.push(Lane::Shutdown, vec![7]), Err(Refused::Full));
}

#[test]
fn a_frame_stays_counted_until_the_writer_has_sent_it() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound.push(Lane::Invoke, vec![0; 10]).expect("invoke");

    let (lane, frame) = outbound.next().expect("the queued frame");
    assert_eq!(lane, Lane::Invoke);
    assert_eq!(
        outbound.held(Lane::Invoke),
        (1, 10),
        "writer-owned, still held"
    );
    assert_eq!(outbound.push(Lane::Invoke, vec![0; 10]), Err(Refused::Full));

    outbound.written(lane, frame.len());
    assert_eq!(outbound.held(Lane::Invoke), (0, 0));
    outbound
        .push(Lane::Invoke, vec![0; 10])
        .expect("room again");
}

#[test]
fn closing_ends_the_writer_and_refuses_frames() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound.push(Lane::Invoke, vec![0; 10]).expect("invoke");
    outbound.close();
    assert!(outbound.next().is_none());
    assert_eq!(outbound.push(Lane::Shutdown, vec![0]), Err(Refused::Closed));
}

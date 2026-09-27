//! Lane isolation: a full lane never takes another lane's room.

use super::{CreditFrame, Lane, LaneBounds, Next, Outbound, Refused};

fn next_frame(outbound: &Outbound) -> super::Queued {
    match outbound.next() {
        Some(Next::Frame(queued)) => queued,
        Some(Next::Credit(credit)) => panic!("expected a frame, got {credit:?}"),
        None => panic!("expected a frame, got the end"),
    }
}

#[test]
fn a_full_invoke_lane_still_admits_cancel_and_shutdown() {
    let outbound = Outbound::new(LaneBounds::derived(2));
    outbound
        .push(Lane::Invoke, vec![1; 10], None)
        .expect("first invoke");
    outbound
        .push(Lane::Invoke, vec![2; 10], None)
        .expect("second invoke");

    assert_eq!(
        outbound.push(Lane::Invoke, vec![3; 10], None),
        Err(Refused::Full),
        "the invoke lane is bounded by the in-flight invocations"
    );
    outbound
        .push(Lane::Cancel, vec![4], None)
        .expect("cancel room");
    outbound
        .push(Lane::Cancel, vec![5], None)
        .expect("cancel room");
    outbound
        .push(Lane::Shutdown, vec![6], None)
        .expect("shutdown room");
    assert_eq!(
        outbound.push(Lane::Shutdown, vec![7], None),
        Err(Refused::Full)
    );
}

#[test]
fn a_frame_stays_counted_until_the_writer_has_sent_it() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound
        .push(Lane::Invoke, vec![0; 10], None)
        .expect("invoke");

    let queued = next_frame(&outbound);
    assert_eq!(queued.lane, Lane::Invoke);
    assert_eq!(
        outbound.held(Lane::Invoke),
        (1, 10),
        "writer-owned, still held"
    );
    assert_eq!(
        outbound.push(Lane::Invoke, vec![0; 10], None),
        Err(Refused::Full)
    );

    outbound.written(queued.lane, queued.frame.len());
    assert_eq!(outbound.held(Lane::Invoke), (0, 0));
    outbound
        .push(Lane::Invoke, vec![0; 10], None)
        .expect("room again");
}

#[test]
fn closing_ends_the_writer_and_refuses_frames() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound
        .push(Lane::Invoke, vec![0; 10], None)
        .expect("invoke");
    outbound.close();
    assert!(outbound.next().is_none());
    assert_eq!(
        outbound.push(Lane::Shutdown, vec![0], None),
        Err(Refused::Closed)
    );
}

#[test]
fn credits_go_before_frames_and_ingress_and_log_credit_coalesce() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound
        .push(Lane::Invoke, vec![0; 10], None)
        .expect("invoke");
    outbound.push_credit(CreditFrame::IngressBytes { bytes: 100 });
    outbound.push_credit(CreditFrame::Delivery { call_id: 7 });
    outbound.push_credit(CreditFrame::Log {
        count: 1,
        bytes: 40,
    });
    outbound.push_credit(CreditFrame::IngressBytes { bytes: 20 });
    outbound.push_credit(CreditFrame::Log {
        count: 2,
        bytes: 60,
    });
    outbound.push_credit(CreditFrame::Reply { call_id: 8 });

    let mut credits = Vec::new();
    while let Some(Next::Credit(credit)) = outbound.next() {
        credits.push(credit);
        if credits.len() == 4 {
            break;
        }
    }
    assert_eq!(
        credits,
        vec![
            CreditFrame::Delivery { call_id: 7 },
            CreditFrame::Reply { call_id: 8 },
            CreditFrame::IngressBytes { bytes: 120 },
            CreditFrame::Log {
                count: 3,
                bytes: 100
            },
        ],
        "ids stay one per credit; ingress and log coalesce into one total each"
    );
    assert_eq!(next_frame(&outbound).lane, Lane::Invoke);
}

#[test]
fn closing_drops_owed_credit() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound.push_credit(CreditFrame::Delivery { call_id: 1 });
    outbound.close();
    outbound.push_credit(CreditFrame::Delivery { call_id: 2 });
    assert!(outbound.next().is_none());
    assert_eq!(outbound.pending_credit_ids(), 0);
}

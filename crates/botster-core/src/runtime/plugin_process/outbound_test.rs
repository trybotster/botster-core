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
fn frames_and_credits_leave_in_push_order_and_ingress_and_log_credit_coalesce() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound.push_credit(CreditFrame::IngressBytes { bytes: 100 });
    outbound
        .push(Lane::Invoke, vec![0; 10], None)
        .expect("invoke");
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

    let mut order = Vec::new();
    for _ in 0..5 {
        order.push(match outbound.next() {
            Some(Next::Credit(credit)) => format!("{credit:?}"),
            Some(Next::Frame(queued)) => format!("{:?}", queued.lane),
            None => panic!("ended early"),
        });
    }
    assert_eq!(
        order,
        vec![
            "IngressBytes { bytes: 120 }".to_string(),
            "Invoke".to_string(),
            "Delivery { call_id: 7 }".to_string(),
            "Log { count: 3, bytes: 100 }".to_string(),
            "Reply { call_id: 8 }".to_string(),
        ],
        "push order; a coalesced total keeps its first position"
    );
}

#[test]
fn a_continuous_credit_stream_cannot_hold_back_cancel_or_shutdown() {
    let outbound = Outbound::new(LaneBounds::derived(1));
    outbound.push_credit(CreditFrame::Log {
        count: 1,
        bytes: 10,
    });
    outbound.push(Lane::Cancel, vec![1], None).expect("cancel");
    outbound
        .push(Lane::Shutdown, vec![2], None)
        .expect("shutdown");
    let mut frames = Vec::new();
    let mut credits = 0;
    while frames.len() < 2 {
        // Each credit the writer takes is replaced at once, as a Hub that
        // keeps draining a chatty plugin would do.
        outbound.push_credit(CreditFrame::Log {
            count: 1,
            bytes: 10,
        });
        match outbound.next() {
            Some(Next::Credit(_)) => credits += 1,
            Some(Next::Frame(queued)) => frames.push(queued.lane),
            None => panic!("ended early"),
        }
        assert!(credits <= 2, "a queued frame waited behind later credit");
    }
    assert_eq!(frames, vec![Lane::Cancel, Lane::Shutdown]);
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

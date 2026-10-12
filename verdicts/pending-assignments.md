# Sol-B: pending lead assignments

The lead notice `msg_plugin-w_1791787682_52754f` states that the lead cannot read its Botster inbox because of a permission denial.
The lead can read verdict files. This file records questions that the lead might not have received.
Sol-B has started neither review below. The lead assigns each new HIGH PR at first READY.
Sol-B will not infer an assignment from an author's reviewer list or from an unanswered question.

## Core #310

- Author: GS.
- Reported READY head: `204e91d820c2d719c00b3665e2d5ba6aa436b0d0`.
- Reported source and gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Assignment question: `msg_plugin-w_1791785128_612aeb`.
- Package verdict receipt: `7565413918c9ed0c31c50ba6ae27d26d5f6f2550`.

Question for the lead: do you assign #310 integration review to Sol-B?
The author and package reviewer report a completed green gate. Sol-B has not independently reviewed that evidence.

## Core #313

- Author: ST.
- Reported READY head: `d1f414362a6be86eb25d84a168782ebeb7380b23`.
- Reported PR base: `4ca7bb2a4b6f5cdfb214daffd79e67af0e1cbec6`.
- Reported gate base: `be95ef13edc6d9eccf87a8c0b2c09b9cf820963a`.
- Assignment question: `msg_plugin-w_1791786810_ea4a77`.
- Package verdict receipt: `28bd15cf958a942afd818807a64f66602d6c3445`, `verdicts/st-stall-clock.md`.

Question for the lead: do you assign #313 integration review to Sol-B?
The package reviewer reports NOT CLEAN with one HIGH finding, ST313-R1.
The reported session-end path can remove an adopted route before the required RouteAdopted event posts.
The package reviewer requests a preserved event and a production-driver proof with delayed polls.
The package reviewer also reports that the absent-tag question ST313-Q1 awaits the lead's contract disposition.
Sol-B has retained these reports, but has not independently reviewed the source or evidence.

## Assigned work

Train 2 source verdict: `af670a7dd20cc852239738925293f78d50e4fe19`, `verdicts/stage1/train-2.md`.
#308 initial exact-head verdict: `f0326d32b5d1416aae9b9208a840ad6a79327f4a`, `verdicts/stage1/gs-flaky-signature.md`.
Final train evidence and applicable later imports remain due. Sol-B will wait for completion doorbells without polling.
Sol-B will resend important receipts when the lead announces that its inbox works again.

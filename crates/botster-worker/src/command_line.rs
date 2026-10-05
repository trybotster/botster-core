//! Command-line decisions with an injected real driver edge.

use botster_core_link::launch::WorkerLaunch;
use std::ffi::OsString;
use std::io;
use std::process::ExitCode;

pub fn execute(
    args: &[OsString],
    token: Option<&str>,
    run: impl FnOnce(&WorkerLaunch) -> io::Result<()>,
) -> (ExitCode, Option<String>) {
    let launch = match WorkerLaunch::parse(args, token) {
        Ok(launch) => launch,
        Err(error) => return (ExitCode::from(2), Some(error.to_string())),
    };
    match run(&launch) {
        Ok(()) => (ExitCode::SUCCESS, None),
        Err(error) => (ExitCode::FAILURE, Some(error.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_contract::prelude::InstanceId;
    use botster_core_link::launch::TOKEN_VAR;

    #[test]
    fn invalid_launch_never_calls_the_driver() {
        let (code, error) = execute(&[], None, |_| panic!("invalid launch reached the driver"));
        assert_eq!(code, ExitCode::from(2));
        assert!(error.is_some_and(|text| !text.is_empty()));
    }

    #[test]
    fn valid_launch_delivers_exact_identity_and_driver_result() {
        let expected = WorkerLaunch {
            control: "/tmp/control".into(),
            instance: InstanceId("1-1".into()),
            host_epoch: 7,
            token: [5; 32],
        };
        let env = expected.env();
        let token = env
            .iter()
            .find(|(name, _)| name == TOKEN_VAR)
            .unwrap()
            .1
            .to_str()
            .unwrap();
        for failed in [false, true] {
            let mut called = false;
            let result = execute(&expected.args(), Some(token), |launch| {
                called = true;
                assert_eq!(launch.control, expected.control);
                assert_eq!(launch.instance, expected.instance);
                assert_eq!(launch.host_epoch, expected.host_epoch);
                assert_eq!(launch.token, expected.token);
                if failed {
                    Err(io::Error::new(
                        io::ErrorKind::ConnectionRefused,
                        "driver refused",
                    ))
                } else {
                    Ok(())
                }
            });
            assert!(called);
            assert_eq!(
                result,
                if failed {
                    (ExitCode::FAILURE, Some("driver refused".into()))
                } else {
                    (ExitCode::SUCCESS, None)
                }
            );
        }
        let result = execute(&expected.args(), None, |_| {
            panic!("missing token reached the driver")
        });
        assert_eq!(result.0, ExitCode::from(2));
        assert!(result.1.is_some_and(|text| !text.is_empty()));
    }
}

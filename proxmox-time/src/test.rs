use anyhow::bail;

use super::*;

fn test_event(v: &'static str) -> Result<(), Error> {
    match parse_calendar_event(v) {
        Ok(event) => println!("CalendarEvent '{}' => {:?}", v, event),
        Err(err) => bail!("parsing '{}' failed - {}", v, err),
    }

    Ok(())
}

const fn make_test_time(mday: i32, hour: i32, min: i32) -> i64 {
    (mday * 3600 * 24 + hour * 3600 + min * 60) as i64
}

#[test]
fn test_compute_next_event() -> Result<(), Error> {
    let test_value = |v: &'static str, last: i64, expect: i64| -> Result<i64, Error> {
        let event = match parse_calendar_event(v) {
            Ok(event) => event,
            Err(err) => bail!("parsing '{}' failed - {}", v, err),
        };

        match event.compute_next_event(last, true) {
            Ok(Some(next)) => {
                if next == expect {
                    println!("next {:?} => {}", event, next);
                } else {
                    bail!(
                        "next {:?} failed\nnext:  {:?}\nexpect: {:?}",
                        event,
                        crate::gmtime(next),
                        crate::gmtime(expect),
                    );
                }
            }
            Ok(None) => bail!("next {:?} failed to find a timestamp", event),
            Err(err) => bail!("compute next for '{}' failed - {}", v, err),
        }

        Ok(expect)
    };

    let test_never = |v: &'static str, last: i64| -> Result<(), Error> {
        let event = match parse_calendar_event(v) {
            Ok(event) => event,
            Err(err) => bail!("parsing '{}' failed - {}", v, err),
        };

        match event.compute_next_event(last, true)? {
            None => Ok(()),
            Some(next) => bail!(
                "compute next for '{}' succeeded, but expected fail - result {}",
                v,
                next
            ),
        }
    };

    const MIN: i64 = 60;
    const HOUR: i64 = 3600;
    const DAY: i64 = 3600 * 24;

    const THURSDAY_00_00: i64 = make_test_time(0, 0, 0);
    const THURSDAY_15_00: i64 = make_test_time(0, 15, 0);

    const JUL_31_2020: i64 = 1596153600; // Friday, 2020-07-31 00:00:00
    const DEC_31_2020: i64 = 1609372800; // Thursday, 2020-12-31 00:00:00

    // minute only syntax
    test_value("0", THURSDAY_00_00, THURSDAY_00_00 + HOUR)?;
    test_value("*", THURSDAY_00_00, THURSDAY_00_00 + MIN)?;

    test_value("*:0", THURSDAY_00_00, THURSDAY_00_00 + HOUR)?;
    test_value("*:*", THURSDAY_00_00, THURSDAY_00_00 + MIN)?;
    test_value("*:*:*", THURSDAY_00_00, THURSDAY_00_00 + 1)?;
    test_value("*:3:5", THURSDAY_00_00, THURSDAY_00_00 + 3 * MIN + 5)?;

    test_value("mon *:*", THURSDAY_00_00, THURSDAY_00_00 + 4 * DAY)?;
    test_value(
        "mon 2:*",
        THURSDAY_00_00,
        THURSDAY_00_00 + 4 * DAY + 2 * HOUR,
    )?;
    test_value(
        "mon 2:50",
        THURSDAY_00_00,
        THURSDAY_00_00 + 4 * DAY + 2 * HOUR + 50 * MIN,
    )?;

    test_value("tue", THURSDAY_00_00, THURSDAY_00_00 + 5 * DAY)?;
    test_value("wed", THURSDAY_00_00, THURSDAY_00_00 + 6 * DAY)?;
    test_value("thu", THURSDAY_00_00, THURSDAY_00_00 + 7 * DAY)?;
    test_value("fri", THURSDAY_00_00, THURSDAY_00_00 + 1 * DAY)?;
    test_value("sat", THURSDAY_00_00, THURSDAY_00_00 + 2 * DAY)?;
    test_value("sun", THURSDAY_00_00, THURSDAY_00_00 + 3 * DAY)?;

    // test repeated ranges
    test_value("4..10/2:0", THURSDAY_00_00, THURSDAY_00_00 + 4 * HOUR)?;
    test_value(
        "4..10/2:0",
        THURSDAY_00_00 + 5 * HOUR,
        THURSDAY_00_00 + 6 * HOUR,
    )?;
    test_value(
        "4..10/2:0",
        THURSDAY_00_00 + 11 * HOUR,
        THURSDAY_00_00 + 1 * DAY + 4 * HOUR,
    )?;

    // test multiple values for a single field
    // and test that the order does not matter
    test_value(
        "5,10:4,8",
        THURSDAY_00_00,
        THURSDAY_00_00 + 5 * HOUR + 4 * MIN,
    )?;
    test_value(
        "10,5:8,4",
        THURSDAY_00_00,
        THURSDAY_00_00 + 5 * HOUR + 4 * MIN,
    )?;
    test_value(
        "6,4..10:23,5/5",
        THURSDAY_00_00,
        THURSDAY_00_00 + 4 * HOUR + 5 * MIN,
    )?;
    test_value(
        "4..10,6:5/5,23",
        THURSDAY_00_00,
        THURSDAY_00_00 + 4 * HOUR + 5 * MIN,
    )?;

    // test month wrapping
    test_value("sat", JUL_31_2020, JUL_31_2020 + 1 * DAY)?;
    test_value("sun", JUL_31_2020, JUL_31_2020 + 2 * DAY)?;
    test_value("mon", JUL_31_2020, JUL_31_2020 + 3 * DAY)?;
    test_value("tue", JUL_31_2020, JUL_31_2020 + 4 * DAY)?;
    test_value("wed", JUL_31_2020, JUL_31_2020 + 5 * DAY)?;
    test_value("thu", JUL_31_2020, JUL_31_2020 + 6 * DAY)?;
    test_value("fri", JUL_31_2020, JUL_31_2020 + 7 * DAY)?;

    // test year wrapping
    test_value("fri", DEC_31_2020, DEC_31_2020 + 1 * DAY)?;
    test_value("sat", DEC_31_2020, DEC_31_2020 + 2 * DAY)?;
    test_value("sun", DEC_31_2020, DEC_31_2020 + 3 * DAY)?;
    test_value("mon", DEC_31_2020, DEC_31_2020 + 4 * DAY)?;
    test_value("tue", DEC_31_2020, DEC_31_2020 + 5 * DAY)?;
    test_value("wed", DEC_31_2020, DEC_31_2020 + 6 * DAY)?;
    test_value("thu", DEC_31_2020, DEC_31_2020 + 7 * DAY)?;

    test_value("daily", THURSDAY_00_00, THURSDAY_00_00 + DAY)?;
    test_value("daily", THURSDAY_00_00 + 1, THURSDAY_00_00 + DAY)?;

    let n = test_value("5/2:0", THURSDAY_00_00, THURSDAY_00_00 + 5 * HOUR)?;
    let n = test_value("5/2:0", n, THURSDAY_00_00 + 7 * HOUR)?;
    let n = test_value("5/2:0", n, THURSDAY_00_00 + 9 * HOUR)?;
    test_value("5/2:0", n, THURSDAY_00_00 + 11 * HOUR)?;

    let mut n = test_value("*:*", THURSDAY_00_00, THURSDAY_00_00 + MIN)?;
    for i in 2..100 {
        n = test_value("*:*", n, THURSDAY_00_00 + i * MIN)?;
    }

    let mut n = test_value("*:0", THURSDAY_00_00, THURSDAY_00_00 + HOUR)?;
    for i in 2..100 {
        n = test_value("*:0", n, THURSDAY_00_00 + i * HOUR)?;
    }

    let mut n = test_value("1:0", THURSDAY_15_00, THURSDAY_00_00 + DAY + HOUR)?;
    for i in 2..100 {
        n = test_value("1:0", n, THURSDAY_00_00 + i * DAY + HOUR)?;
    }

    // test date functionality

    test_value("2020-07-31", 0, JUL_31_2020)?;
    test_value("02-28", 0, (31 + 27) * DAY)?;
    test_value("02-29", 0, 2 * 365 * DAY + (31 + 28) * DAY)?; // 1972-02-29
    test_value("1965/5-01-01", -1, THURSDAY_00_00)?;
    test_value("2020-7..9-2/2", JUL_31_2020, JUL_31_2020 + 2 * DAY)?;
    test_value("2020,2021-12-31", JUL_31_2020, DEC_31_2020)?;

    test_value("monthly", 0, 31 * DAY)?;
    test_value("quarterly", 0, (31 + 28 + 31) * DAY)?;
    test_value("semiannually", 0, (31 + 28 + 31 + 30 + 31 + 30) * DAY)?;
    test_value("yearly", 0, (365) * DAY)?;

    test_never("2021-02-29", 0)?;
    test_never("02-30", 0)?;

    Ok(())
}

#[test]
fn test_calendar_event_weekday() -> Result<(), Error> {
    test_event("mon,wed..fri")?;
    test_event("fri..mon")?;

    test_event("mon")?;
    test_event("MON")?;
    test_event("monDay")?;
    test_event("tue")?;
    test_event("Tuesday")?;
    test_event("wed")?;
    test_event("wednesday")?;
    test_event("thu")?;
    test_event("thursday")?;
    test_event("fri")?;
    test_event("friday")?;
    test_event("sat")?;
    test_event("saturday")?;
    test_event("sun")?;
    test_event("sunday")?;

    test_event("mon..fri")?;
    test_event("mon,tue,fri")?;
    test_event("mon,tue..wednesday,fri..sat")?;

    Ok(())
}

#[test]
fn test_time_span_parser() -> Result<(), Error> {
    let test_value = |ts_str: &str, expect: f64| -> Result<(), Error> {
        let ts = parse_time_span(ts_str)?;
        assert_eq!(f64::from(ts), expect, "{}", ts_str);
        Ok(())
    };

    test_value("2", 2.0)?;
    test_value("2s", 2.0)?;
    test_value("2sec", 2.0)?;
    test_value("2second", 2.0)?;
    test_value("2seconds", 2.0)?;

    test_value(" 2s 2 s 2", 6.0)?;

    test_value("1msec 1ms", 0.002)?;
    test_value("1usec 1us 1µs", 0.000_003)?;
    test_value("1nsec 1ns", 0.000_000_002)?;
    test_value("1minutes 1minute 1min 1m", 4.0 * 60.0)?;
    test_value("1hours 1hour 1hr 1h", 4.0 * 3600.0)?;
    test_value("1days 1day 1d", 3.0 * 86400.0)?;
    test_value("1weeks 1 week 1w", 3.0 * 86400.0 * 7.0)?;
    test_value("1months 1month 1M", 3.0 * 86400.0 * 30.44)?;
    test_value("1years 1year 1y", 3.0 * 86400.0 * 365.25)?;

    test_value("2h", 7200.0)?;
    test_value(" 2 h", 7200.0)?;
    test_value("2hours", 7200.0)?;
    test_value("48hr", 48.0 * 3600.0)?;
    test_value(
        "1y 12month",
        365.25 * 24.0 * 3600.0 + 12.0 * 30.44 * 24.0 * 3600.0,
    )?;
    test_value("55s500ms", 55.5)?;
    test_value("300ms20s 5day", 5.0 * 24.0 * 3600.0 + 20.0 + 0.3)?;

    Ok(())
}

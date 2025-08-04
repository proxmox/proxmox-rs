use anyhow::{anyhow, bail, Context, Error};

const VALID_DAYS_OF_WEEK: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
const VALID_MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

#[derive(Debug, PartialEq)]
/// Last modified timestamp as obtained from API response http headers.
pub struct LastModifiedTimestamp {
    _datetime: iso8601::DateTime,
}

impl std::str::FromStr for LastModifiedTimestamp {
    type Err = Error;

    fn from_str(timestamp: &str) -> Result<Self, Self::Err> {
        let _datetime = iso8601::datetime(timestamp).map_err(|err| anyhow!(err))?;
        Ok(Self { _datetime })
    }
}

serde_plain::derive_deserialize_from_fromstr!(LastModifiedTimestamp, "last modified timestamp");

/// Preferred date format specified by RFC2616, given as fixed-length
/// subset of RFC1123, which itself is a followup to RFC822.
///
/// https://datatracker.ietf.org/doc/html/rfc2616#section-3.3
/// https://datatracker.ietf.org/doc/html/rfc1123#section-5.2.14
/// https://datatracker.ietf.org/doc/html/rfc822#section-5
#[derive(Debug)]
pub struct HttpDate {
    _epoch: i64,
}

impl std::str::FromStr for HttpDate {
    type Err = Error;

    fn from_str(timestamp: &str) -> Result<Self, Self::Err> {
        let input = timestamp.as_bytes();
        if input.len() != 29 {
            bail!("unexpected length: got {}", input.len());
        }

        let expect = |pos: usize, c: u8| {
            if input[pos] != c {
                bail!("unexpected char at pos {pos}");
            }
            Ok(())
        };

        let digit = |pos: usize| -> Result<i32, Error> {
            let digit = input[pos] as i32;
            if !(48..=57).contains(&digit) {
                bail!("unexpected char at pos {pos}");
            }
            Ok(digit - 48)
        };

        fn check_max(i: i32, max: i32) -> Result<i32, Error> {
            if i > max {
                bail!("value too large ({i} > {max})");
            }
            Ok(i)
        }

        let mut tm = proxmox_time::TmEditor::new(true);

        if !VALID_DAYS_OF_WEEK
            .iter()
            .any(|valid| valid.as_bytes() == &input[0..3])
        {
            bail!("unexpected day of week, got {:?}", &input[0..3]);
        }

        expect(3, b',').context("unexpected separator after day of week")?;
        expect(4, b' ').context("missing space after day of week separator")?;
        tm.set_mday(check_max(digit(5)? * 10 + digit(6)?, 31)?)?;
        expect(7, b' ').context("unexpected separator after day")?;
        if let Some(month) = VALID_MONTHS
            .iter()
            .position(|month| month.as_bytes() == &input[8..11])
        {
            // valid conversion to i32, position stems from fixed size array of 12 months.
            tm.set_mon(check_max(month as i32 + 1, 12)?)?;
        } else {
            bail!("invalid month");
        }
        expect(11, b' ').context("unexpected separator after month")?;
        tm.set_year(digit(12)? * 1000 + digit(13)? * 100 + digit(14)? * 10 + digit(15)?)?;
        expect(16, b' ').context("unexpected separator after year")?;
        tm.set_hour(check_max(digit(17)? * 10 + digit(18)?, 23)?)?;
        expect(19, b':').context("unexpected separator after hour")?;
        tm.set_min(check_max(digit(20)? * 10 + digit(21)?, 59)?)?;
        expect(22, b':').context("unexpected separator after minute")?;
        tm.set_sec(check_max(digit(23)? * 10 + digit(24)?, 60)?)?;
        expect(25, b' ').context("unexpected separator after second")?;
        if !input.ends_with(b"GMT") {
            bail!("unexpected timezone");
        }

        let _epoch = tm.into_epoch()?;

        Ok(Self { _epoch })
    }
}

use std::process::{Command, Stdio};

use anyhow::Error;

use super::{SyslogFilter, SyslogLine};

pub fn dump_journal(filter: SyslogFilter) -> Result<(u64, Vec<SyslogLine>), Error> {
    let mut args = vec!["-o", "short", "--no-pager"];

    if let Some(service) = &filter.service {
        args.extend(["--unit", service]);
    }
    if let Some(since) = &filter.since {
        args.extend(["--since", since]);
    }
    if let Some(until) = &filter.until {
        args.extend(["--until", until]);
    }

    let mut lines: Vec<SyslogLine> = Vec::new();
    let mut limit = filter.limit.unwrap_or(50);
    let start = filter.start.unwrap_or(0);
    let mut count: u64 = 0;

    let mut child = Command::new("journalctl")
        .args(&args)
        .stdout(Stdio::piped())
        .spawn()?;

    use std::io::{BufRead, BufReader};

    if let Some(ref mut stdout) = child.stdout {
        for line in BufReader::new(stdout).lines() {
            match line {
                Ok(line) => {
                    count += 1;
                    if count < start {
                        continue;
                    };
                    if limit == 0 {
                        continue;
                    };

                    lines.push(SyslogLine { n: count, t: line });

                    limit -= 1;
                }
                Err(err) => {
                    log::error!("reading journal failed: {}", err);
                    let _ = child.kill();
                    break;
                }
            }
        }
    }

    let status = child.wait().unwrap();
    if !status.success() {
        log::error!("journalctl failed with {}", status);
    }

    // HACK: ExtJS store.guaranteeRange() does not like empty array
    // so we add a line
    if count == 0 {
        count += 1;
        lines.push(SyslogLine {
            n: count,
            t: String::from("no content"),
        });
    }

    Ok((count, lines))
}

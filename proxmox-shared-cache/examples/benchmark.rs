use std::time::{Duration, Instant};

use anyhow::Error;

use proxmox_shared_cache::SharedCache;
use proxmox_sys::fs::CreateOptions;
use serde_json::{Map, Number, Value};

const CACHE_SIZE: u32 = 180;

fn make_cache() -> Result<SharedCache, Error> {
    let options = CreateOptions::new()
        .owner(nix::unistd::Uid::effective())
        .group(nix::unistd::Gid::effective())
        .perm(nix::sys::stat::Mode::from_bits_truncate(0o600));

    let cache = SharedCache::new("/tmp/proxmox_shared_cache", options, CACHE_SIZE)?;
    Ok(cache)
}

fn make_map(keys: u32) -> Value {
    let mut map = Map::new();

    for i in 0..keys {
        map.insert(format!("key_{i}"), Value::Number(Number::from(i)));
    }

    Value::Object(map)
}

fn test_set(value: &Value) -> Vec<u64> {
    let cache = make_cache().expect("could not make cache");

    let mut durations = Vec::new();

    for _ in 0..20 {
        let now = Instant::now();
        cache
            .set(&value, Duration::from_secs(1))
            .expect("could not set value");
        let elapsed = now.elapsed();
        durations.push(elapsed.as_micros() as u64);
    }

    durations
}

fn test_get_last(last: u32) -> Vec<u64> {
    let cache = make_cache().expect("could not make cache");

    let mut durations = Vec::new();

    for _ in 0..20 {
        let now = Instant::now();
        let _val: Vec<Map<String, Value>> = cache.get_last(last).expect("could not get value");
        let elapsed = now.elapsed();
        durations.push(elapsed.as_micros() as u64);
    }

    durations
}

fn test_get() -> Vec<u64> {
    let cache = make_cache().expect("could not make cache");

    let mut durations = Vec::new();

    for _ in 0..20 {
        let now = Instant::now();
        let _val: Option<Map<String, Value>> = cache.get().expect("could not get value");
        let elapsed = now.elapsed();
        durations.push(elapsed.as_micros() as u64);
    }

    durations
}

fn prepare_cache(value: &Value) -> Result<(), Error> {
    let cache = make_cache()?;

    cache.delete(Duration::from_secs(1))?;
    for _ in 0..CACHE_SIZE {
        cache.set(value, Duration::from_secs(1))?;
    }

    Ok(())
}

fn print_results(tag: &str, durations: &[u64]) {
    let sum: u64 = durations.iter().sum::<u64>();

    let n = durations.len() as u64;
    let avg = sum / n;

    let mut s = 0f64;
    for duration in durations {
        let a = (*duration as f64 - avg as f64).powf(2.0);
        let a = a.sqrt();

        s += a;
    }

    let variance = s / (n as f64);

    println!("{tag:20}: {avg} ± {variance} µs");
}

fn main() -> Result<(), Error> {
    for num_keys in [100, 1000, 10000] {
        println!("\nValue: {num_keys} keys");
        println!("-------------------------------");

        let value = make_map(num_keys);
        for last in [10, 100] {
            prepare_cache(&value)?;

            let res = test_get_last(last);
            print_results(&format!("get_last {last}"), &res);
        }
        prepare_cache(&value)?;

        let res = test_set(&value);
        print_results("set", &res);

        let res = test_get();
        print_results("get", &res);
    }

    Ok(())
}

//! What workerd has spent: the summed CPU time and resident memory of every `workerd serve`
//! process, read from `ps` (the tenant Worker's `measure.sh` method).

use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub cpu_ms: u64,
    pub rss_kib: u64,
}

fn seconds(cputime: &str) -> Option<f64> {
    let mut total = 0f64;
    for part in cputime.split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

pub fn sample() -> Result<Sample, String> {
    let out = Command::new("ps")
        .args(["-axo", "time=,rss=,command="])
        .output()
        .map_err(|e| format!("ps: {e}"))?;
    let text = String::from_utf8_lossy(&out.stdout);
    let (mut cpu, mut rss, mut found) = (0f64, 0u64, false);
    for line in text.lines().filter(|l| l.contains("workerd serve")) {
        let mut fields = line.split_whitespace();
        let (Some(time), Some(kib)) = (fields.next(), fields.next()) else {
            continue;
        };
        cpu += seconds(time).ok_or_else(|| format!("unreadable cputime {time}"))?;
        rss += kib.parse::<u64>().map_err(|e| e.to_string())?;
        found = true;
    }
    if !found {
        return Err("no workerd process is running".into());
    }
    Ok(Sample {
        cpu_ms: (cpu * 1000.0) as u64,
        rss_kib: rss,
    })
}

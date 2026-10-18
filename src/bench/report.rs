use serde::Serialize;

#[derive(Serialize)]
pub struct Summary {
    pub samples: Vec<f64>,
    pub median: f64,
    pub p95: f64,
}
pub fn summary(samples: Vec<f64>) -> Result<Summary, String> {
    if samples.is_empty() || samples.iter().any(|x| !x.is_finite() || *x < 0.) {
        return Err("Invalid measurement samples".into());
    }
    let mut sorted = samples.clone();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    let median = if n.is_multiple_of(2) {
        sorted[n / 2 - 1] / 2. + sorted[n / 2] / 2.
    } else {
        sorted[n / 2]
    };
    Ok(Summary {
        samples,
        median,
        p95: sorted[(n * 95).div_ceil(100) - 1],
    })
}
fn escape(text: &str) -> String {
    let mut escaped = String::new();
    for c in text.chars() {
        escaped.push_str(match c {
            '&' => "&amp;",
            '<' => "&lt;",
            '>' => "&gt;",
            '"' => "&quot;",
            '\'' => "&#39;",
            _ => {
                escaped.push(c);
                continue;
            }
        });
    }
    escaped
}
pub fn html(json: &str) -> String {
    let escaped = escape(json);
    let mut rows = String::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
        if let Some(summaries) = value["summaries"].as_array() {
            for row in summaries {
                rows.push_str("<tr><th scope=\"row\">");
                rows.push_str(&escape(row["workload"].as_str().unwrap_or("unknown")));
                rows.push_str("</th>");
                for field in [
                    "startup_ms",
                    "idle_private_mib",
                    "after_private_mib",
                    "parent_cpu_seconds",
                    "elapsed_seconds",
                    "response_proxy_ms",
                ] {
                    rows.push_str("<td>");
                    match row[field]["median"].as_f64() {
                        Some(number) if number.is_finite() => {
                            rows.push_str(&format!("{number:.2}"))
                        }
                        _ => rows.push_str("unmeasured"),
                    }
                    rows.push_str("</td>");
                }
                rows.push_str("</tr>");
            }
        }
    }
    format!("<!doctype html><html lang=\"en-CA\"><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width\"><title>Juna benchmark</title><style>body{{background:#1e1e2e;color:#cdd6f4;font:16px system-ui;max-width:1100px;margin:3rem auto;padding:1rem}}pre{{white-space:pre-wrap;overflow-wrap:anywhere}}h1{{color:#cba6f7}}table{{border-collapse:collapse;width:100%}}th,td{{padding:.6rem;text-align:left;border-bottom:1px solid #585b70}}.table{{overflow-x:auto}}</style><h1>Juna benchmark evidence</h1><p>Median values. Response times include software capture and coordination overhead. They do not establish physical display latency. Small sample counts are exploratory.</p><div class=\"table\"><table><thead><tr><th>Workload</th><th>Warm startup ms</th><th>Idle private MiB</th><th>After private MiB</th><th>Parent CPU s</th><th>Elapsed s</th><th>Response proxy ms</th></tr></thead><tbody>{rows}</tbody></table></div><details><summary>Raw samples, p95 and limitations</summary><pre>{escaped}</pre></details></html>")
}
pub fn marker(epoch: u32) -> [u8; 3] {
    // Distinct, well-separated colours for up to 64 requests. No two match within tolerance.
    [
        40 + (epoch % 4) as u8 * 48,
        35 + ((epoch / 4) % 4) as u8 * 48,
        30 + ((epoch / 16) % 4) as u8 * 48,
    ]
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn statistics_and_rejection() {
        let s = summary(vec![4., 1., 3., 2.]).unwrap();
        assert_eq!(s.median, 2.5);
        assert_eq!(s.p95, 4.);
        assert_eq!(s.samples, [4., 1., 3., 2.]);
        assert_eq!(summary(vec![f64::MAX, f64::MAX]).unwrap().median, f64::MAX);
        for v in [vec![], vec![f64::NAN], vec![f64::INFINITY], vec![-1.]] {
            assert!(summary(v).is_err());
        }
    }
    #[test]
    fn report_cannot_inject_markup() {
        let h = html("</pre><script>\"&'");
        assert!(!h.contains("<script>"));
        assert!(h.contains("&lt;/pre&gt;&lt;script&gt;&quot;&amp;&#39;"));
    }
    #[test]
    fn epochs_cannot_match_stale_frames() {
        for a in 0..64 {
            for b in 0..64 {
                if a != b {
                    assert!(marker(a)
                        .into_iter()
                        .zip(marker(b))
                        .any(|(x, y)| x.abs_diff(y) > 6));
                }
            }
        }
    }
}

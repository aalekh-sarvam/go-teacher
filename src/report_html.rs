//! Render the Markdown report as a styled HTML page for the in-app viewer.

use pulldown_cmark::{html, Options, Parser};

const CSS: &str = r#"
  :root { --bg:#f6f4ee; --fg:#222; --muted:#666; --card:#fff; --line:#ddd; --accent:#2f6f4f; }
  * { box-sizing:border-box; }
  body { margin:0; padding:24px 16px 60px; font:15px/1.55 -apple-system, BlinkMacSystemFont, "Segoe UI", Helvetica, Arial, sans-serif; background:var(--bg); color:var(--fg); }
  main { max-width:960px; margin:0 auto; background:var(--card); border:1px solid var(--line); border-radius:10px; padding:28px 34px; }
  nav { max-width:960px; margin:0 auto 14px; display:flex; gap:16px; align-items:center; font-size:14px; }
  nav a { color:var(--accent); text-decoration:none; font-weight:600; }
  h1 { font-size:26px; margin:0 0 12px; }
  h2 { font-size:20px; margin:34px 0 10px; padding-top:14px; border-top:1px solid var(--line); }
  h3 { font-size:16px; margin:26px 0 8px; color:#333; }
  table { border-collapse:collapse; margin:10px 0 16px; font-size:13.5px; max-width:100%; display:block; overflow-x:auto; }
  th, td { border:1px solid var(--line); padding:5px 9px; text-align:left; vertical-align:top; white-space:nowrap; }
  th { background:#f0efe9; }
  td:last-child { white-space:normal; }
  pre { background:#fbf8f1; border:1px solid #e6dfcf; border-radius:8px; padding:12px 14px; overflow-x:auto; font:13px/1.25 "SF Mono", Menlo, Consolas, monospace; }
  code { font:0.92em "SF Mono", Menlo, Consolas, monospace; background:#f0efe9; padding:1px 4px; border-radius:4px; }
  pre code { background:none; padding:0; }
  blockquote { margin:10px 0; padding:6px 14px; border-left:4px solid var(--accent); color:#444; background:#f7f7f3; }
  li { margin:3px 0; }
  strong { color:#111; }
  .toc { font-size:13px; color:var(--muted); }
"#;

/// Turn "Move N" mentions in headings and bullets into links that open the position in the live board.
fn link_moves(markdown: &str, job_id: u64) -> String {
    let mut out = String::with_capacity(markdown.len() + 1024);
    let mut in_code = false;
    for line in markdown.lines() {
        if line.starts_with("```") {
            in_code = !in_code;
            out.push_str(line);
            out.push('\n');
            continue;
        }
        if in_code {
            out.push_str(line);
            out.push('\n');
            continue;
        }
        let mut l = line.to_string();
        // "### Move 15: Black G4" and "### Candidate: move 15 (Black G4)"
        for prefix in ["### Move ", "### Candidate: move ", "- **Move "] {
            if let Some(rest) = l.strip_prefix(prefix) {
                let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
                if !digits.is_empty() {
                    let tail = &rest[digits.len()..];
                    let (open, close) = if prefix == "- **Move " { ("- **", "**") } else { (prefix, "") };
                    let tail = if prefix == "- **Move " { tail.strip_prefix("**").unwrap_or(tail) } else { tail };
                    l = format!("{}[Move {}](/?job={}&turn={}){}{}", open.trim_end_matches("Move ").trim_end_matches("move "), digits, job_id, digits, close, tail);
                }
                break;
            }
        }
        out.push_str(&l);
        out.push('\n');
    }
    out
}

pub fn render_page(file_name: &str, markdown: &str, job_id: u64) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    let linked = link_moves(markdown, job_id);
    let parser = Parser::new_ext(&linked, opts);
    let mut body = String::with_capacity(markdown.len() * 2);
    html::push_html(&mut body, parser);
    // The machine block is available on demand; it should not dominate the report viewer.
    if let Some(start) = body.find("<pre><code class=\"language-go-teacher-evidence\">") {
        if let Some(relative_end) = body[start..].find("</code></pre>") {
            let end = start + relative_end + "</code></pre>".len();
            body.insert_str(end, "</details>");
            body.insert_str(start, "<details><summary>Structured evidence for the teaching skill</summary>");
        }
    }
    let title = format!("Review of {}", html_escape(file_name));
    format!(
        "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
<title>{title}</title><style>{CSS}</style></head><body>\
<nav><a href=\"/\">&larr; Go Teacher</a><span class=\"toc\">{title}</span>\
<span style=\"flex:1\"></span>\
<a href=\"/api/jobs/{job_id}/lesson-bundle\" onclick=\"fetch(this.href,{{method:'POST'}}).then(r=>r.text()).then(t=>alert(t));return false;\">lesson bundle</a>\
<a href=\"/api/jobs/{job_id}/detailed.md\">detailed report</a> <a href=\"/api/jobs/{job_id}/report.md?inline=1\">raw Markdown</a></nav>\
<main>{body}</main></body></html>"
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

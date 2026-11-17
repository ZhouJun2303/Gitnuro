use std::{collections::HashMap, path::Path, sync::{atomic::{AtomicU64, Ordering}, Mutex, OnceLock}, time::Instant};
use serde::Serialize;

struct Scan { root: String, scan: awegit_git::RepositoryScan, used: Instant }
fn scans() -> &'static Mutex<HashMap<String, Scan>> {
    static SCANS: OnceLock<Mutex<HashMap<String, Scan>>> = OnceLock::new();
    SCANS.get_or_init(Default::default)
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Page { paths: Vec<String>, truncated: bool, cursor: Option<String> }

pub fn page(root: String, depth: Option<u32>, cursor: Option<String>) -> Result<Page, String> {
    super::lab::ensure_lab_path(Path::new(&root))?;
    let mut scans = scans().lock().unwrap_or_else(|error| error.into_inner());
    scans.retain(|_, scan| scan.used.elapsed().as_secs() < 300);
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let id = cursor.unwrap_or_else(|| format!("scan-{}", NEXT.fetch_add(1, Ordering::Relaxed)));
    if !scans.contains_key(&id) {
        if scans.len() >= 16 { return Err("Too many active directory scans".into()); }
        scans.insert(id.clone(), Scan { root: root.clone(), scan: awegit_git::RepositoryScan::new(Path::new(&root), depth.unwrap_or(5) as usize).map_err(|error| error.to_string())?, used: Instant::now() });
    }
    let scan = scans.get_mut(&id).unwrap();
    if scan.root != root { return Err("Scan source changed".into()); }
    scan.used = Instant::now();
    let (result, more) = scan.scan.next_page();
    if !more { scans.remove(&id); }
    Ok(Page { paths: result.paths, truncated: result.truncated, cursor: more.then_some(id) })
}

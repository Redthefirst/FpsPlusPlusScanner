use std::{path::PathBuf, sync::Arc};
use tokio::sync::{Semaphore, mpsc};
use walkdir::WalkDir;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("1) Scan all drives\n2) Scan People Playground\nChoose 1 or 2:");
    let mut choice = String::new();
    std::io::stdin().read_line(&mut choice)?;

    let roots: Vec<PathBuf> = if choice.trim() == "1" {
        (b'A'..=b'Z')
            .map(|letter| PathBuf::from(format!("{}:\\", letter as char)))
            .filter(|path| path.is_dir())
            .collect()
    } else {
        let path =
            PathBuf::from(r"C:\Program Files (x86)\Steam\steamapps\common\People Playground");
        if path.is_dir() {
            vec![path]
        } else {
            Vec::new()
        }
    };

    if roots.is_empty() {
        eprintln!("No scan directory was found.");
        return Ok(());
    }

    let (paths_tx, mut paths_rx) = mpsc::channel::<PathBuf>(256);
    let walker = tokio::task::spawn_blocking(move || {
        for root in roots {
            for entry in WalkDir::new(root)
                .follow_links(false)
                .into_iter()
                .filter_map(Result::ok)
            {
                if entry.file_type().is_file() && paths_tx.blocking_send(entry.into_path()).is_err()
                {
                    return;
                }
            }
        }
    });

    let workers = Arc::new(Semaphore::new(8));
    let mut scans = Vec::new();
    while let Some(path) = paths_rx.recv().await {
        let permit = Arc::clone(&workers).acquire_owned().await?;
        scans.push(tokio::spawn(async move {
            let result = tokio::task::spawn_blocking(move || {
                let bytes = std::fs::read(&path).ok()?;
                let found = [
                    &b"GsAUgBsAGUAcABKADIAZABtAGUARQA1ADcANQArAC8AWABhA"[..],
                    &b"HAASgAyAGQAbQBlAEUANQA3ADUAKwAvAFgAYQBlADAATQBzA"[..],
                ]
                .iter()
                .any(|signature| {
                    bytes
                        .windows(signature.len())
                        .any(|window| window == *signature)
                        || bytes.windows(signature.len() * 2).any(|window| {
                            window
                                .chunks_exact(2)
                                .zip(*signature)
                                .all(|(pair, byte)| pair == [*byte, 0])
                        })
                });
                found.then_some(path)
            })
            .await;
            drop(permit);
            result.ok().flatten()
        }));
    }

    walker.await?;
    for scan in scans {
        if let Some(path) = scan.await? {
            println!(
                "Bad: loader signature found in {} Likely means your infected",
                path.display()
            );
        }
    }
    Ok(())
}
